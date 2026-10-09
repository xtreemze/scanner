import ARKit
import AVFoundation
import CoreMotion
import Foundation
import simd

struct IOSNativeCapabilities: Codable {
  let camera: Bool
  let imu: Bool
  let sceneDepth: Bool
  let smoothedSceneDepth: Bool
  let lidar: Bool
  let sceneMesh: Bool
  let flashHardware: Bool
  let hdrVideo: Bool
}

struct IOSPoseSample: Codable {
  let positionMeters: [Double]
  let orientationXyzw: [Double]
}

struct IOSCameraIntrinsics: Codable {
  let widthPx: Int
  let heightPx: Int
  let fx: Double
  let fy: Double
  let cx: Double
  let cy: Double
}

struct IOSDepthDescriptor: Codable {
  let widthPx: Int
  let heightPx: Int
  let hasConfidence: Bool
  let smoothed: Bool
}

struct IOSFrameMetadata: Codable {
  let timestampMicros: UInt64
  let poseDeviceLocal: IOSPoseSample
  let intrinsics: IOSCameraIntrinsics
  let exposureDurationSeconds: Double
  let exposureOffsetEv: Double
  let trackingState: String
  let depth: IOSDepthDescriptor?
}

struct IOSMotionSample: Codable {
  let timestampMicros: UInt64
  let userAccelerationMps2: [Double]
  let rotationRateRps: [Double]
  let gravityMps2: [Double]
}

/// Native iOS acquisition adapter.
///
/// This class intentionally stops at the platform boundary. It exposes frame metadata and
/// zero-copy depth/image buffers to a future FFI ingestion layer; it does not make ARKit's
/// coordinate system authoritative for Scanner.
final class ScannerSensorAdapter: NSObject, ARSessionDelegate {
  let session = ARSession()
  private let motionManager = CMMotionManager()

  var onFrameMetadata: ((IOSFrameMetadata) -> Void)?
  var onMotionSample: ((IOSMotionSample) -> Void)?
  var ingestBridge: ScannerCoreIngestBridge?

  override init() {
    super.init()
    session.delegate = self
  }

  static func capabilities() -> IOSNativeCapabilities {
    let rearCamera = AVCaptureDevice.default(
      .builtInWideAngleCamera,
      for: .video,
      position: .back
    )
    let sceneDepth = ARWorldTrackingConfiguration.supportsFrameSemantics(.sceneDepth)
    let smoothedDepth = ARWorldTrackingConfiguration.supportsFrameSemantics(.smoothedSceneDepth)
    let sceneMesh = ARWorldTrackingConfiguration.supportsSceneReconstruction(.mesh)

    return IOSNativeCapabilities(
      camera: rearCamera != nil,
      imu: CMMotionManager().isDeviceMotionAvailable,
      sceneDepth: sceneDepth,
      smoothedSceneDepth: smoothedDepth,
      // ARKit scene depth is currently exposed on LiDAR-capable world-facing devices.
      lidar: sceneDepth,
      sceneMesh: sceneMesh,
      flashHardware: rearCamera?.hasTorch == true,
      hdrVideo: rearCamera?.formats.contains(where: { $0.isVideoHDRSupported }) == true
    )
  }

  func start(resetTracking: Bool = false) throws {
    guard ARWorldTrackingConfiguration.isSupported else {
      throw ScannerSensorAdapterError.worldTrackingUnsupported
    }

    let configuration = ARWorldTrackingConfiguration()
    configuration.worldAlignment = .gravity
    configuration.planeDetection = [.horizontal, .vertical]

    var semantics: ARConfiguration.FrameSemantics = []
    if ARWorldTrackingConfiguration.supportsFrameSemantics(.sceneDepth) {
      semantics.insert(.sceneDepth)
    }
    if ARWorldTrackingConfiguration.supportsFrameSemantics(.smoothedSceneDepth) {
      semantics.insert(.smoothedSceneDepth)
    }
    configuration.frameSemantics = semantics

    if ARWorldTrackingConfiguration.supportsSceneReconstruction(.mesh) {
      configuration.sceneReconstruction = .mesh
    }

    let options: ARSession.RunOptions = resetTracking
      ? [.resetTracking, .removeExistingAnchors]
      : []
    session.run(configuration, options: options)
    startMotionUpdates()
  }

  func stop() {
    motionManager.stopDeviceMotionUpdates()
    session.pause()
  }

  func setTorch(level: Float?) throws {
    guard let device = AVCaptureDevice.default(
      .builtInWideAngleCamera,
      for: .video,
      position: .back
    ), device.hasTorch else {
      throw ScannerSensorAdapterError.torchUnavailable
    }

    try device.lockForConfiguration()
    defer { device.unlockForConfiguration() }

    if let level {
      let bounded = min(max(level, 0.01), 1.0)
      try device.setTorchModeOn(level: bounded)
    } else {
      device.torchMode = .off
    }
  }

  func session(_ session: ARSession, didUpdate frame: ARFrame) {
    let camera = frame.camera
    let transform = camera.transform
    let rotation = simd_quatf(transform)

    let intrinsics = camera.intrinsics
    let resolution = camera.imageResolution

    let depthData: (ARDepthData, Bool)? = {
      if let raw = frame.sceneDepth {
        return (raw, false)
      }
      if let smoothed = frame.smoothedSceneDepth {
        return (smoothed, true)
      }
      return nil
    }()

    let depthDescriptor = depthData.map { depth, smoothed in
      IOSDepthDescriptor(
        widthPx: CVPixelBufferGetWidth(depth.depthMap),
        heightPx: CVPixelBufferGetHeight(depth.depthMap),
        hasConfidence: depth.confidenceMap != nil,
        smoothed: smoothed
      )
    }

    let metadata = IOSFrameMetadata(
      timestampMicros: UInt64(max(frame.timestamp, 0) * 1_000_000),
      poseDeviceLocal: IOSPoseSample(
        positionMeters: [
          Double(transform.columns.3.x),
          Double(transform.columns.3.y),
          Double(transform.columns.3.z),
        ],
        orientationXyzw: [
          Double(rotation.imag.x),
          Double(rotation.imag.y),
          Double(rotation.imag.z),
          Double(rotation.real),
        ]
      ),
      intrinsics: IOSCameraIntrinsics(
        widthPx: Int(resolution.width),
        heightPx: Int(resolution.height),
        fx: Double(intrinsics.columns.0.x),
        fy: Double(intrinsics.columns.1.y),
        cx: Double(intrinsics.columns.2.x),
        cy: Double(intrinsics.columns.2.y)
      ),
      exposureDurationSeconds: camera.exposureDuration,
      exposureOffsetEv: Double(camera.exposureOffset),
      trackingState: Self.trackingState(camera.trackingState),
      depth: depthDescriptor
    )

    onFrameMetadata?(metadata)
    ingestBridge?.ingest(frameMetadata: metadata, depthData: depthData?.0)

    // frame.capturedImage, depthData?.0.depthMap and confidenceMap deliberately remain
    // native pixel buffers here. The high-rate ingestion path should pass them directly
    // to native/Rust processing instead of serializing them through the Tauri WebView.
  }

  private func startMotionUpdates() {
    guard motionManager.isDeviceMotionAvailable else { return }

    motionManager.deviceMotionUpdateInterval = 1.0 / 100.0
    motionManager.startDeviceMotionUpdates(to: .main) { [weak self] motion, _ in
      guard let self, let motion else { return }
      let gravityScale = 9.80665
      let sample = IOSMotionSample(
        timestampMicros: UInt64(max(motion.timestamp, 0) * 1_000_000),
        userAccelerationMps2: [
          motion.userAcceleration.x * gravityScale,
          motion.userAcceleration.y * gravityScale,
          motion.userAcceleration.z * gravityScale,
        ],
        rotationRateRps: [
          motion.rotationRate.x,
          motion.rotationRate.y,
          motion.rotationRate.z,
        ],
        gravityMps2: [
          motion.gravity.x * gravityScale,
          motion.gravity.y * gravityScale,
          motion.gravity.z * gravityScale,
        ]
      )
      self.onMotionSample?(sample)
      self.ingestBridge?.ingest(motion: sample)
    }
  }

  private static func trackingState(_ state: ARCamera.TrackingState) -> String {
    switch state {
    case .normal:
      return "normal"
    case .notAvailable:
      return "not-available"
    case .limited(let reason):
      switch reason {
      case .initializing:
        return "limited-initializing"
      case .excessiveMotion:
        return "limited-excessive-motion"
      case .insufficientFeatures:
        return "limited-insufficient-features"
      case .relocalizing:
        return "limited-relocalizing"
      @unknown default:
        return "limited-unknown"
      }
    }
  }
}

enum ScannerSensorAdapterError: Error {
  case worldTrackingUnsupported
  case torchUnavailable
}
