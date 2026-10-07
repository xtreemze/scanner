# Native mobile sensor adapters

This directory contains the platform acquisition layer that will be wired into the Tauri 2 mobile host.

The source is intentionally separate from `scanner-core`:

- ARKit, AVFoundation and Core Motion stay on the iOS side.
- ARCore, Camera2 and Android sensor APIs stay on the Android side.
- platform frames are translated into Scanner observation contracts before they can affect canonical state.
- `SessionWorld` remains authoritative; ARKit/ARCore world origins are device-local evidence.

## Control plane versus data plane

Tauri's mobile plugin mechanism is appropriate for low-rate commands such as:

- capability negotiation;
- start/stop capture;
- requesting permissions;
- changing supported camera/illumination controls;
- reporting adapter state.

High-rate camera, depth and IMU payloads must not be serialized through the WebView. The intended data path is native Swift/Kotlin -> FFI/JNI -> Rust ingestion/reconstruction, while Tauri remains the UI/control host.

## iOS baseline

`ios/ScannerSensorAdapter.swift` currently defines:

- ARKit world tracking setup;
- sceneDepth/smoothedSceneDepth capability checks;
- scene-mesh capability detection;
- camera pose/intrinsics/exposure metadata normalization;
- depth/confidence descriptors without copying pixel buffers;
- Core Motion acquisition;
- hardware torch control boundary.

It has not yet been compiled or device-certified in the Tauri iOS project.

## Android baseline

`android/ScannerSensorAdapter.kt` currently defines:

- ARCore support/depth capability probing;
- RAW_DEPTH_ONLY preference with AUTOMATIC fallback;
- camera pose/intrinsics/timestamp normalization;
- raw 16-bit depth + 8-bit confidence lifetime handling;
- explicit detection of fresh versus reprojected raw depth via timestamps;
- accelerometer/gyroscope acquisition.

Use ARCore SDK `com.google.ar:core:1.56.0` when wiring the Android library. The project is AR Optional: camera-only operation must remain possible.

It has not yet been compiled or device-certified in the Tauri Android project.

## Verification boundary

These files are platform source baselines, not evidence that the native adapters work on hardware. Issues #2 and #3 remain open until the Tauri mobile projects are wired, compiled, and physically tested according to `docs/verification/certification-matrix.md`.
