package com.xtreemze.scanner.sensors

import android.app.Activity
import android.content.Context
import android.content.pm.PackageManager
import android.hardware.Sensor
import android.hardware.SensorEvent
import android.hardware.SensorEventListener
import android.hardware.SensorManager
import android.media.Image
import android.os.Build
import com.google.ar.core.ArCoreApk
import com.google.ar.core.Config
import com.google.ar.core.Frame
import com.google.ar.core.Session
import com.google.ar.core.ArCoreApk.InstallStatus
import com.google.ar.core.TrackingState
import kotlin.math.max

data class AndroidNativeCapabilities(
    val camera: Boolean,
    val imu: Boolean,
    val arCoreSupported: Boolean,
    val rawDepth: Boolean,
    val flashHardware: Boolean,
)

data class AndroidPoseSample(
    val positionMeters: DoubleArray,
    val orientationXyzw: DoubleArray,
)

data class AndroidCameraIntrinsics(
    val widthPx: Int,
    val heightPx: Int,
    val fx: Double,
    val fy: Double,
    val cx: Double,
    val cy: Double,
)

data class AndroidFrameMetadata(
    val frameTimestampNanos: Long,
    val cameraTimestampNanos: Long,
    val poseDeviceLocal: AndroidPoseSample,
    val intrinsics: AndroidCameraIntrinsics,
    val trackingState: String,
)

data class AndroidDepthDescriptor(
    val widthPx: Int,
    val heightPx: Int,
    val timestampNanos: Long,
    val isFreshForFrame: Boolean,
)

data class AndroidMotionSample(
    val sensorTimestampNanos: Long,
    val accelerationMps2: DoubleArray?,
    val angularVelocityRps: DoubleArray?,
)

data class AndroidFrameIngestReport(
    val cameraStatus: Int?,
    val depthStatus: Int?,
    val depthFresh: Boolean,
)

/// Native Android acquisition adapter.
///
/// The ARCore render/update loop owns Frame creation. This adapter configures the Session and
/// normalizes current-frame metadata. Raw image buffers are supplied to a callback and closed
/// immediately after the callback returns; they should be consumed by JNI/native code rather than
/// copied through the WebView.
class ScannerSensorAdapter(private val activity: Activity) : SensorEventListener {
    private val sensorManager =
        activity.getSystemService(Context.SENSOR_SERVICE) as SensorManager

    private var latestAcceleration: DoubleArray? = null
    private var latestAngularVelocity: DoubleArray? = null

    var onMotionSample: ((AndroidMotionSample) -> Unit)? = null

    fun capabilities(session: Session? = null): AndroidNativeCapabilities {
        val packageManager = activity.packageManager
        val arCoreSupported =
            ArCoreApk.getInstance().checkAvailability(activity).isSupported
        val rawDepth = session?.isDepthModeSupported(Config.DepthMode.RAW_DEPTH_ONLY) == true

        return AndroidNativeCapabilities(
            camera = packageManager.hasSystemFeature(PackageManager.FEATURE_CAMERA_ANY),
            imu = sensorManager.getDefaultSensor(Sensor.TYPE_ACCELEROMETER) != null &&
                sensorManager.getDefaultSensor(Sensor.TYPE_GYROSCOPE) != null,
            arCoreSupported = arCoreSupported,
            rawDepth = rawDepth,
            flashHardware = packageManager.hasSystemFeature(PackageManager.FEATURE_CAMERA_FLASH),
        )
    }

    fun ensureArCoreReady(): Boolean =
        when (ArCoreApk.getInstance().requestInstall(activity, true)) {
            InstallStatus.INSTALLED -> true
            InstallStatus.INSTALL_REQUESTED -> false
        }

    fun configureSession(
        session: Session,
        preferRawDepth: Boolean = true,
        useNativeFramePump: Boolean = true,
    ): Config {
        val config = session.config

        config.depthMode = when {
            preferRawDepth && session.isDepthModeSupported(Config.DepthMode.RAW_DEPTH_ONLY) ->
                Config.DepthMode.RAW_DEPTH_ONLY
            session.isDepthModeSupported(Config.DepthMode.AUTOMATIC) ->
                Config.DepthMode.AUTOMATIC
            session.isDepthModeSupported(Config.DepthMode.RAW_DEPTH_ONLY) ->
                Config.DepthMode.RAW_DEPTH_ONLY
            else -> Config.DepthMode.DISABLED
        }

        config.updateMode = Config.UpdateMode.LATEST_CAMERA_IMAGE

        if (useNativeFramePump) {
            require(Build.VERSION.SDK_INT >= Build.VERSION_CODES.O_MR1) {
                "Native ARCore frame pump requires Android 8.1/API 27 or newer"
            }
            config.textureUpdateMode = Config.TextureUpdateMode.EXPOSE_HARDWARE_BUFFER
        }

        session.configure(config)
        return config
    }

    fun frameMetadata(frame: Frame): AndroidFrameMetadata {
        val camera = frame.camera
        val pose = camera.pose
        val intrinsics = camera.imageIntrinsics
        val focalLength = FloatArray(2)
        val principalPoint = FloatArray(2)
        val dimensions = IntArray(2)

        intrinsics.getFocalLength(focalLength, 0)
        intrinsics.getPrincipalPoint(principalPoint, 0)
        intrinsics.getImageDimensions(dimensions, 0)

        return AndroidFrameMetadata(
            frameTimestampNanos = frame.timestamp,
            cameraTimestampNanos = frame.androidCameraTimestamp,
            poseDeviceLocal = AndroidPoseSample(
                positionMeters = doubleArrayOf(
                    pose.tx().toDouble(),
                    pose.ty().toDouble(),
                    pose.tz().toDouble(),
                ),
                orientationXyzw = doubleArrayOf(
                    pose.qx().toDouble(),
                    pose.qy().toDouble(),
                    pose.qz().toDouble(),
                    pose.qw().toDouble(),
                ),
            ),
            intrinsics = AndroidCameraIntrinsics(
                widthPx = dimensions[0],
                heightPx = dimensions[1],
                fx = focalLength[0].toDouble(),
                fy = focalLength[1].toDouble(),
                cx = principalPoint[0].toDouble(),
                cy = principalPoint[1].toDouble(),
            ),
            trackingState = when (camera.trackingState) {
                TrackingState.TRACKING -> "tracking"
                TrackingState.PAUSED -> "paused"
                TrackingState.STOPPED -> "stopped"
            },
        )
    }

    fun ingestFrame(frame: Frame, bridge: ScannerCoreIngestBridge): AndroidFrameIngestReport {
        if (frame.camera.trackingState != TrackingState.TRACKING) {
            return AndroidFrameIngestReport(
                cameraStatus = null,
                depthStatus = null,
                depthFresh = false,
            )
        }

        val cameraStatus = bridge.ingestCamera(frameMetadata(frame))
        var depthStatus: Int? = null
        var depthFresh = false

        withRawDepth(frame) { depth, confidence, descriptor ->
            depthFresh = descriptor.isFreshForFrame
            if (!descriptor.isFreshForFrame) return@withRawDepth

            val depthPlane = depth.planes.singleOrNull() ?: return@withRawDepth
            val confidencePlane = confidence.planes.singleOrNull() ?: return@withRawDepth

            depthStatus = bridge.ingestDepth(
                descriptor = descriptor,
                depth = depthPlane.buffer,
                depthRowStrideBytes = depthPlane.rowStride,
                confidence = confidencePlane.buffer,
                confidenceRowStrideBytes = confidencePlane.rowStride,
            )
        }

        return AndroidFrameIngestReport(
            cameraStatus = cameraStatus,
            depthStatus = depthStatus,
            depthFresh = depthFresh,
        )
    }

    fun withRawDepth(
        frame: Frame,
        consumer: (depth: Image, confidence: Image, descriptor: AndroidDepthDescriptor) -> Unit,
    ): Boolean {
        val depth = try {
            frame.acquireRawDepthImage16Bits()
        } catch (_: Exception) {
            return false
        }

        val confidence = try {
            frame.acquireRawDepthConfidenceImage()
        } catch (_: Exception) {
            depth.close()
            return false
        }

        try {
            val descriptor = AndroidDepthDescriptor(
                widthPx = depth.width,
                heightPx = depth.height,
                timestampNanos = depth.timestamp,
                isFreshForFrame = depth.timestamp == frame.timestamp,
            )
            consumer(depth, confidence, descriptor)
            return true
        } finally {
            confidence.close()
            depth.close()
        }
    }

    fun startImu(rateMicros: Int = 10_000) {
        val boundedRate = max(rateMicros, 2_500)
        sensorManager.getDefaultSensor(Sensor.TYPE_ACCELEROMETER)?.let {
            sensorManager.registerListener(this, it, boundedRate)
        }
        sensorManager.getDefaultSensor(Sensor.TYPE_GYROSCOPE)?.let {
            sensorManager.registerListener(this, it, boundedRate)
        }
    }

    fun stopImu() {
        sensorManager.unregisterListener(this)
    }

    override fun onSensorChanged(event: SensorEvent) {
        when (event.sensor.type) {
            Sensor.TYPE_ACCELEROMETER -> {
                latestAcceleration = doubleArrayOf(
                    event.values[0].toDouble(),
                    event.values[1].toDouble(),
                    event.values[2].toDouble(),
                )
            }
            Sensor.TYPE_GYROSCOPE -> {
                latestAngularVelocity = doubleArrayOf(
                    event.values[0].toDouble(),
                    event.values[1].toDouble(),
                    event.values[2].toDouble(),
                )
            }
            else -> return
        }

        onMotionSample?.invoke(
            AndroidMotionSample(
                sensorTimestampNanos = event.timestamp,
                accelerationMps2 = latestAcceleration,
                angularVelocityRps = latestAngularVelocity,
            )
        )
    }

    override fun onAccuracyChanged(sensor: Sensor?, accuracy: Int) = Unit
}
