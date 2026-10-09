import AVFoundation
import SwiftRs
import Tauri

struct StartSessionArgs: Decodable {
  let deviceId: String
  let epoch: UInt64
  let resetTracking: Bool?
  let preferRawDepth: Bool?
}

struct TorchArgs: Decodable {
  let level: Float?
}

final class ScannerSensorsPlugin: Plugin {
  private let adapter = ScannerSensorAdapter()

  @objc public func capabilities(_ invoke: Invoke) {
    let capabilities = ScannerSensorAdapter.capabilities()
    var result: JsonObject = [:]
    result["camera"] = capabilities.camera
    result["imu"] = capabilities.imu
    result["depth"] = capabilities.sceneDepth
    result["lidar"] = capabilities.lidar
    result["sceneMesh"] = capabilities.sceneMesh
    result["flashHardware"] = capabilities.flashHardware
    // Hardware presence is not equivalent to safe torch control while ARKit owns capture.
    result["flashSessionControl"] = false
    result["hdr"] = capabilities.hdrVideo
    result["platformTracking"] = capabilities.camera
    invoke.resolve(result)
  }

  @objc override public func checkPermissions(_ invoke: Invoke) {
    let state: String
    switch AVCaptureDevice.authorizationStatus(for: .video) {
    case .authorized:
      state = "granted"
    case .denied, .restricted:
      state = "denied"
    case .notDetermined:
      state = "prompt"
    @unknown default:
      state = "prompt"
    }
    invoke.resolve(["camera": state])
  }

  @objc override public func requestPermissions(_ invoke: Invoke) {
    switch AVCaptureDevice.authorizationStatus(for: .video) {
    case .notDetermined:
      AVCaptureDevice.requestAccess(for: .video) { [weak self] _ in
        self?.checkPermissions(invoke)
      }
    default:
      checkPermissions(invoke)
    }
  }

  @objc public func startSession(_ invoke: Invoke) throws {
    guard AVCaptureDevice.authorizationStatus(for: .video) == .authorized else {
      invoke.reject("Camera permission is required before starting ARKit")
      return
    }

    let args = try invoke.parseArgs(StartSessionArgs.self)
    guard !args.deviceId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
      invoke.reject("A non-empty deviceId is required")
      return
    }
    guard let bridge = ScannerCoreIngestBridge(deviceId: args.deviceId, epoch: args.epoch) else {
      invoke.reject("Unable to create scanner-core ingestion session")
      return
    }

    adapter.ingestBridge = bridge
    do {
      try adapter.start(resetTracking: args.resetTracking ?? false)
      invoke.resolve()
    } catch {
      bridge.close()
      adapter.ingestBridge = nil
      invoke.reject("Unable to start ARKit session: \(error.localizedDescription)")
    }
  }

  @objc public func stopSession(_ invoke: Invoke) {
    adapter.stop()
    adapter.ingestBridge?.close()
    adapter.ingestBridge = nil
    invoke.resolve()
  }

  @objc public func setTorch(_ invoke: Invoke) throws {
    _ = try invoke.parseArgs(TorchArgs.self)
    // Do not call the AVFoundation torch while ARKit owns the camera until the interaction
    // has been physically validated. The source adapter retains the low-level boundary.
    invoke.reject("Torch control is not yet verified for an active ARKit session")
  }
}

@_cdecl("init_plugin_scanner_sensors")
func initPlugin() -> Plugin {
  ScannerSensorsPlugin()
}
