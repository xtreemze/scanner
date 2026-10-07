# ADR 0012: native adapter control plane and high-rate data plane

Status: accepted

## Decision

Use Tauri 2 mobile plugins as Scanner's native control plane, but do not route high-rate reconstruction data through the WebView.

### Control plane

Swift and Kotlin plugin commands may provide:

- capability negotiation;
- permission state;
- capture session start/stop;
- capture configuration;
- controlled illumination commands;
- low-rate diagnostics and state transitions.

Tauri's native mobile plugin model supports Swift on iOS and Kotlin/Java on Android and provides the correct host boundary for these commands.

### Data plane

Camera pixel buffers, depth/confidence images, point clouds, and high-rate IMU samples are performance-sensitive source evidence.

Their target path is:

    ARKit / ARCore / platform sensors
      -> native adapter
      -> FFI (iOS) / JNI (Android)
      -> Rust ingestion and SessionWorld/reconstruction
      -> bounded presentation telemetry
      -> Tauri/Lit UI

Raw frame payloads must not make a native -> WebView -> Rust round trip.

## iOS

Use ARKit `ARWorldTrackingConfiguration` for device-local visual-inertial pose. Enable `sceneDepth` and `smoothedSceneDepth` only after `supportsFrameSemantics` succeeds. Preserve ARDepthData's confidence map where available.

ARKit's world origin is never SessionWorld.

## Android

Treat ARCore as optional capability rather than an application requirement.

Prefer `Config.DepthMode.RAW_DEPTH_ONLY` for reconstruction when supported, with `AUTOMATIC` as a fallback. Raw depth is sparse and paired with an 8-bit confidence image. A raw-depth timestamp equal to the current frame timestamp means new depth evidence; repeated timestamps represent reprojected depth and must not be counted as independent measurements.

The initial Android integration target is ARCore SDK 1.56.0.

## Camera control

A device having flash hardware does not prove that flash/torch can safely be controlled while ARKit/ARCore owns the camera. Scanner therefore distinguishes physical capability discovery from verified session-controllable illumination.

No native capability is exposed to the product UI until its adapter has a working implementation for the current session.

## Verification

The source adapters may land before device certification, but issues #2 and #3 remain open until:

- Tauri iOS/Android mobile projects build;
- permissions and lifecycle are integrated;
- timestamps are mapped through the Scanner clock model;
- raw data reaches the Rust ingestion path;
- camera/torch interactions are physically tested;
- required devices in the certification matrix have evidence.
