package com.xtreemze.scanner.sensors

import java.nio.ByteBuffer

/**
 * Thin JNI data-plane bridge into scanner-core.
 *
 * Raw camera/depth/IMU payloads never cross the WebView. The native shim resolves the
 * platform-neutral scanner_core C ABI at runtime and returns ScannerNativeStatus values.
 */
class ScannerCoreIngestBridge private constructor(private var nativeHandle: Long) : AutoCloseable {
    data class Stats(
        val acceptedCameraFrames: Long,
        val acceptedImuSamples: Long,
        val acceptedDepthFrames: Long,
        val droppedObservations: Long,
        val droppedDepthFrames: Long,
        val queuedObservations: Long,
        val queuedDepthFrames: Long,
    )

    val isOpen: Boolean
        get() = nativeHandle != 0L

    fun ingestCamera(metadata: AndroidFrameMetadata): Int {
        val pose = metadata.poseDeviceLocal
        val intrinsics = metadata.intrinsics
        return nativeIngestCamera(
            nativeHandle,
            metadata.cameraTimestampNanos / 1_000L,
            CLOCK_CAMERA_SENSOR,
            CAMERA_UNCERTAINTY_MICROS,
            pose.positionMeters[0],
            pose.positionMeters[1],
            pose.positionMeters[2],
            pose.orientationXyzw[0],
            pose.orientationXyzw[1],
            pose.orientationXyzw[2],
            pose.orientationXyzw[3],
            intrinsics.widthPx,
            intrinsics.heightPx,
            intrinsics.fx,
            intrinsics.fy,
            intrinsics.cx,
            intrinsics.cy,
        )
    }

    fun ingestMotion(sample: AndroidMotionSample): Int {
        val acceleration = sample.accelerationMps2 ?: return STATUS_INVALID_METADATA
        val angularVelocity = sample.angularVelocityRps ?: return STATUS_INVALID_METADATA
        return nativeIngestImu(
            nativeHandle,
            sample.sensorTimestampNanos / 1_000L,
            CLOCK_DEVICE_MONOTONIC,
            IMU_UNCERTAINTY_MICROS,
            acceleration[0],
            acceleration[1],
            acceleration[2],
            angularVelocity[0],
            angularVelocity[1],
            angularVelocity[2],
        )
    }

    fun ingestDepth(
        descriptor: AndroidDepthDescriptor,
        depth: ByteBuffer,
        depthRowStrideBytes: Int,
        confidence: ByteBuffer,
        confidenceRowStrideBytes: Int,
    ): Int {
        require(depth.isDirect) { "Depth ByteBuffer must be direct" }
        require(confidence.isDirect) { "Confidence ByteBuffer must be direct" }
        return nativeIngestDepth(
            nativeHandle,
            descriptor.timestampNanos / 1_000L,
            CLOCK_CAMERA_SENSOR,
            DEPTH_UNCERTAINTY_MICROS,
            descriptor.widthPx,
            descriptor.heightPx,
            DEPTH_U16_MILLIMETERS,
            depth,
            depthRowStrideBytes,
            confidence,
            confidenceRowStrideBytes,
            descriptor.isFreshForFrame,
        )
    }

    fun stats(): Stats? {
        val values = LongArray(7)
        val status = nativeStats(nativeHandle, values)
        if (status != STATUS_OK) return null
        return Stats(
            acceptedCameraFrames = values[0],
            acceptedImuSamples = values[1],
            acceptedDepthFrames = values[2],
            droppedObservations = values[3],
            droppedDepthFrames = values[4],
            queuedObservations = values[5],
            queuedDepthFrames = values[6],
        )
    }

    override fun close() {
        val handle = nativeHandle
        nativeHandle = 0L
        if (handle != 0L) nativeDestroy(handle)
    }

    companion object {
        const val STATUS_OK = 0
        const val STATUS_INVALID_METADATA = 5

        private const val CLOCK_DEVICE_MONOTONIC = 1
        private const val CLOCK_CAMERA_SENSOR = 2
        private const val DEPTH_U16_MILLIMETERS = 1

        // Conservative initial uncertainty until device-specific clock calibration is available.
        private const val CAMERA_UNCERTAINTY_MICROS = 1_000
        private const val DEPTH_UNCERTAINTY_MICROS = 1_000
        private const val IMU_UNCERTAINTY_MICROS = 500

        init {
            System.loadLibrary("scanner_sensors_jni")
        }

        fun create(deviceId: String, epoch: Long): ScannerCoreIngestBridge? {
            if (deviceId.isBlank() || epoch < 0) return null
            val handle = nativeCreate(deviceId, epoch)
            return if (handle == 0L) null else ScannerCoreIngestBridge(handle)
        }

        @JvmStatic private external fun nativeCreate(deviceId: String, epoch: Long): Long
        @JvmStatic private external fun nativeDestroy(handle: Long)

        @JvmStatic
        private external fun nativeIngestCamera(
            handle: Long,
            timestampMicros: Long,
            clockDomain: Int,
            uncertaintyMicros: Int,
            px: Double,
            py: Double,
            pz: Double,
            qx: Double,
            qy: Double,
            qz: Double,
            qw: Double,
            widthPx: Int,
            heightPx: Int,
            fx: Double,
            fy: Double,
            cx: Double,
            cy: Double,
        ): Int

        @JvmStatic
        private external fun nativeIngestImu(
            handle: Long,
            timestampMicros: Long,
            clockDomain: Int,
            uncertaintyMicros: Int,
            ax: Double,
            ay: Double,
            az: Double,
            gx: Double,
            gy: Double,
            gz: Double,
        ): Int

        @JvmStatic
        private external fun nativeIngestDepth(
            handle: Long,
            timestampMicros: Long,
            clockDomain: Int,
            uncertaintyMicros: Int,
            widthPx: Int,
            heightPx: Int,
            depthFormat: Int,
            depth: ByteBuffer,
            depthRowStrideBytes: Int,
            confidence: ByteBuffer,
            confidenceRowStrideBytes: Int,
            freshForFrame: Boolean,
        ): Int

        @JvmStatic private external fun nativeStats(handle: Long, out: LongArray): Int
    }
}
