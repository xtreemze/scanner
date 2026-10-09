package com.xtreemze.scanner.sensors

import android.Manifest
import android.app.Activity
import app.tauri.PermissionState
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import com.google.ar.core.Session

@InvokeArg
class StartSessionArgs {
    var deviceId: String = ""
    var epoch: Long = 0
    var resetTracking: Boolean = false
    var preferRawDepth: Boolean = true
}

@InvokeArg
class TorchArgs {
    var level: Float? = null
}

@TauriPlugin(
    permissions = [
        Permission(strings = [Manifest.permission.CAMERA], alias = "camera")
    ]
)
class ScannerSensorsPlugin(private val activity: Activity) : Plugin(activity) {
    private val adapter = ScannerSensorAdapter(activity)
    private var session: Session? = null
    private var ingestBridge: ScannerCoreIngestBridge? = null

    @Command
    fun capabilities(invoke: Invoke) {
        val capabilities = adapter.capabilities(session)
        val result = JSObject()
        result.put("camera", capabilities.camera)
        result.put("imu", capabilities.imu)
        result.put("depth", capabilities.rawDepth)
        result.put("lidar", false)
        result.put("sceneMesh", false)
        result.put("flashHardware", capabilities.flashHardware)
        // Camera-session torch control has not yet been proven safe with ARCore.
        result.put("flashSessionControl", false)
        result.put("hdr", false)
        result.put("platformTracking", capabilities.arCoreSupported)
        invoke.resolve(result)
    }

    @Command
    override fun checkPermissions(invoke: Invoke) {
        val result = JSObject()
        result.put("camera", when (getPermissionState("camera")) {
            PermissionState.GRANTED -> "granted"
            PermissionState.DENIED -> "denied"
            else -> "prompt"
        })
        invoke.resolve(result)
    }

    @Command
    override fun requestPermissions(invoke: Invoke) {
        if (getPermissionState("camera") == PermissionState.GRANTED) {
            checkPermissions(invoke)
            return
        }
        requestPermissionForAlias("camera", invoke, "cameraPermissionCallback")
    }

    @app.tauri.annotation.PermissionCallback
    private fun cameraPermissionCallback(invoke: Invoke) {
        checkPermissions(invoke)
    }

    @Command
    fun startSession(invoke: Invoke) {
        if (getPermissionState("camera") != PermissionState.GRANTED) {
            invoke.reject("Camera permission is required before starting ARCore")
            return
        }

        val args = invoke.parseArgs(StartSessionArgs::class.java)
        if (args.deviceId.isBlank() || args.epoch < 0) {
            invoke.reject("A non-empty deviceId and non-negative epoch are required")
            return
        }

        try {
            val activeSession = session ?: Session(activity).also { session = it }
            val bridge = ScannerCoreIngestBridge.create(args.deviceId, args.epoch)
                ?: throw IllegalStateException("scanner-core ingestion ABI is unavailable")

            ingestBridge?.close()
            ingestBridge = bridge
            adapter.onMotionSample = { sample ->
                bridge.ingestMotion(sample)
            }

            adapter.configureSession(activeSession)
            activeSession.resume()
            adapter.startImu()
            invoke.resolve()
        } catch (error: Exception) {
            invoke.reject("Unable to start ARCore session: ${error.message ?: error.javaClass.simpleName}")
        }
    }

    @Command
    fun stopSession(invoke: Invoke) {
        adapter.stopImu()
        adapter.onMotionSample = null
        session?.pause()
        ingestBridge?.close()
        ingestBridge = null
        invoke.resolve()
    }

    /**
     * Called by the native ARCore update/render loop after Session.update().
     *
     * This is deliberately not a Tauri command: high-rate camera/depth payloads must not route
     * through the WebView. The host integration should call this method directly for each Frame.
     */
    fun ingestFrame(frame: com.google.ar.core.Frame): AndroidFrameIngestReport? {
        val bridge = ingestBridge ?: return null
        return adapter.ingestFrame(frame, bridge)
    }

    @Command
    fun ingestStats(invoke: Invoke) {
        val stats = ingestBridge?.stats()
        if (stats == null) {
            invoke.reject("No active scanner-core ingestion session")
            return
        }

        val result = JSObject()
        result.put("acceptedCameraFrames", stats.acceptedCameraFrames)
        result.put("acceptedImuSamples", stats.acceptedImuSamples)
        result.put("acceptedDepthFrames", stats.acceptedDepthFrames)
        result.put("droppedObservations", stats.droppedObservations)
        result.put("droppedDepthFrames", stats.droppedDepthFrames)
        result.put("queuedObservations", stats.queuedObservations)
        result.put("queuedDepthFrames", stats.queuedDepthFrames)
        invoke.resolve(result)
    }

    @Command
    fun setTorch(invoke: Invoke) {
        // ARCore owns the camera device while its Session is active. Do not claim torch
        // support until a camera-session-safe control path has been device verified.
        invoke.reject("Torch control is not yet verified for an active ARCore session")
    }

    override fun onPause() {
        super.onPause()
        adapter.stopImu()
        session?.pause()
    }

    override fun onResume() {
        super.onResume()
        session?.let {
            try {
                it.resume()
                adapter.startImu()
            } catch (_: Exception) {
                // The control command will surface a concrete failure on the next explicit start.
            }
        }
    }
}
