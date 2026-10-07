import ARKit
import CoreVideo
import Foundation
import ScannerCoreFFI

/// Direct high-rate bridge from Apple sensor callbacks into scanner-core.
///
/// Tauri remains the control plane. This object does not emit frame/depth/IMU payloads through
/// the WebView.
final class ScannerCoreIngestBridge {
  private var handle: OpaquePointer?

  init?(deviceId: String, epoch: UInt64) {
    guard !deviceId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
      return nil
    }

    let created: OpaquePointer? = deviceId.utf8CString.withUnsafeBufferPointer { buffer in
      guard let baseAddress = buffer.baseAddress else { return nil }
      let bytes = UnsafeRawPointer(baseAddress).assumingMemoryBound(to: UInt8.self)
      return scanner_native_session_create(bytes, deviceId.utf8.count, epoch)
    }

    guard let created else { return nil }
    handle = created
  }

  deinit {
    close()
  }

  func close() {
    guard let handle else { return }
    scanner_native_session_destroy(handle)
    self.handle = nil
  }

  func ingest(frameMetadata frame: IOSFrameMetadata, depthData: ARDepthData?) {
    guard let handle else { return }
    guard frame.poseDeviceLocal.positionMeters.count == 3,
          frame.poseDeviceLocal.orientationXyzw.count == 4 else {
      return
    }

    let position = ScannerNativeVec3(
      x: frame.poseDeviceLocal.positionMeters[0],
      y: frame.poseDeviceLocal.positionMeters[1],
      z: frame.poseDeviceLocal.positionMeters[2]
    )
    let orientation = ScannerNativeQuaternion(
      x: frame.poseDeviceLocal.orientationXyzw[0],
      y: frame.poseDeviceLocal.orientationXyzw[1],
      z: frame.poseDeviceLocal.orientationXyzw[2],
      w: frame.poseDeviceLocal.orientationXyzw[3]
    )
    let pose = ScannerNativePose(
      position_meters: position,
      orientation: orientation
    )
    let intrinsics = ScannerNativeCameraIntrinsics(
      width_px: UInt32(clamping: frame.intrinsics.widthPx),
      height_px: UInt32(clamping: frame.intrinsics.heightPx),
      fx: frame.intrinsics.fx,
      fy: frame.intrinsics.fy,
      cx: frame.intrinsics.cx,
      cy: frame.intrinsics.cy
    )

    // ARCamera exposes exposure duration/offset but not frame-synchronous ISO. scanner-core uses
    // zero for "not supplied by this platform adapter"; a later calibrated camera path can add it.
    var metadata = ScannerNativeCameraFrameMetadata(
      timestamp_micros: frame.timestampMicros,
      clock_domain: 2,
      uncertainty_micros: 0,
      pose_device_local: pose,
      intrinsics: intrinsics,
      exposure_seconds: frame.exposureDurationSeconds,
      iso: 0,
      aperture_f_number: 0,
      white_balance_kelvin: 0,
      optional_fields: 0
    )

    _ = withUnsafePointer(to: &metadata) {
      scanner_native_ingest_camera_metadata(handle, $0)
    }

    if let depthData {
      ingest(depthData: depthData, timestampMicros: frame.timestampMicros)
    }
  }

  func ingest(motion: IOSMotionSample) {
    guard let handle else { return }
    guard motion.userAccelerationMps2.count == 3,
          motion.rotationRateRps.count == 3,
          motion.gravityMps2.count == 3 else {
      return
    }

    // Core Motion userAcceleration excludes gravity. Recombine it so acceleration_mps2 has the
    // same semantics as a raw accelerometer sample while also retaining gravity independently.
    let acceleration = ScannerNativeVec3(
      x: motion.userAccelerationMps2[0] + motion.gravityMps2[0],
      y: motion.userAccelerationMps2[1] + motion.gravityMps2[1],
      z: motion.userAccelerationMps2[2] + motion.gravityMps2[2]
    )
    let angularVelocity = ScannerNativeVec3(
      x: motion.rotationRateRps[0],
      y: motion.rotationRateRps[1],
      z: motion.rotationRateRps[2]
    )
    let gravity = ScannerNativeVec3(
      x: motion.gravityMps2[0],
      y: motion.gravityMps2[1],
      z: motion.gravityMps2[2]
    )
    var sample = ScannerNativeImuSample(
      timestamp_micros: motion.timestampMicros,
      clock_domain: 1,
      uncertainty_micros: 0,
      acceleration_mps2: acceleration,
      angular_velocity_rps: angularVelocity,
      gravity_mps2: gravity,
      has_gravity: 1
    )

    _ = withUnsafePointer(to: &sample) {
      scanner_native_ingest_imu(handle, $0)
    }
  }

  private func ingest(depthData: ARDepthData, timestampMicros: UInt64) {
    guard let handle else { return }

    let depth = depthData.depthMap
    let confidence = depthData.confidenceMap

    CVPixelBufferLockBaseAddress(depth, .readOnly)
    defer { CVPixelBufferUnlockBaseAddress(depth, .readOnly) }

    guard let depthBase = CVPixelBufferGetBaseAddress(depth) else { return }

    var confidenceBase: UnsafeMutableRawPointer?
    var confidenceLength = 0
    var confidenceRowStride = 0

    if let confidence {
      CVPixelBufferLockBaseAddress(confidence, .readOnly)
      confidenceBase = CVPixelBufferGetBaseAddress(confidence)
      confidenceLength = CVPixelBufferGetBytesPerRow(confidence) * CVPixelBufferGetHeight(confidence)
      confidenceRowStride = CVPixelBufferGetBytesPerRow(confidence)
    }
    defer {
      if let confidence {
        CVPixelBufferUnlockBaseAddress(confidence, .readOnly)
      }
    }

    let width = CVPixelBufferGetWidth(depth)
    let height = CVPixelBufferGetHeight(depth)
    let depthRowStride = CVPixelBufferGetBytesPerRow(depth)
    let depthLength = depthRowStride * height

    var descriptor = ScannerNativeDepthDescriptor(
      timestamp_micros: timestampMicros,
      clock_domain: UInt32(SCANNER_CLOCK_CAMERA_SENSOR),
      uncertainty_micros: 0,
      width_px: UInt32(clamping: width),
      height_px: UInt32(clamping: height),
      depth_format: 2,
      depth_row_stride_bytes: depthRowStride,
      confidence_row_stride_bytes: confidenceRowStride,
      // Zero/zero asks scanner-core to derive the valid range while it copies the depth evidence.
      min_depth_meters: 0,
      max_depth_meters: 0,
      fresh_for_frame: 1
    )

    let depthBytes = depthBase.assumingMemoryBound(to: UInt8.self)
    let confidenceBytes = confidenceBase?.assumingMemoryBound(to: UInt8.self)

    _ = withUnsafePointer(to: &descriptor) {
      scanner_native_ingest_depth(
        handle,
        $0,
        depthBytes,
        depthLength,
        confidenceBytes,
        confidenceLength
      )
    }
  }
}
