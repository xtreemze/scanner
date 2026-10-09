# ADR 0014: Android JNI uses direct buffers into scanner-core

Status: accepted

## Decision

The Android native data plane uses a thin C JNI shim to call the existing scanner-core C ABI.

Kotlin owns Android/ARCore DTOs and image lifetimes. The shim receives only primitive metadata and direct ByteBuffers, then immediately calls scanner-core. Rust validates/copies accepted evidence into its bounded ingestion queues before the JNI call returns.

The WebView and Tauri command channel are not used for high-rate camera, depth, confidence, or IMU payloads.

## Rust library ownership

Android does not create a second scanner-core runtime.

The Tauri Android app library already links scanner-core. The app explicitly retains the scanner-core ingestion ABI symbols, and the JNI shim resolves those symbols from the process library libscanner_lib.so with dlsym.

This preserves one Rust state owner and avoids separately loading another scanner-core shared library with independent globals/lifecycle.

## Frame lifecycle

ARCore still owns Session.update() and Frame creation. The plugin exposes a native-only ingestFrame(frame) hook intended to be called by the Android ARCore update/render integration.

Only TRACKING frames are admitted.

Raw depth is admitted only when its timestamp matches the current ARCore frame. Reprojected/repeated depth is not counted as a new independent depth observation.

Depth and confidence buffers are passed as direct buffers with explicit row stride. Rust copies accepted bytes before ARCore Image objects are closed.

## Clock domains

- ARCore camera/depth timestamps enter as camera-sensor time.
- SensorEvent timestamps enter as device-monotonic time.
- Initial uncertainty is conservative until device-specific clock calibration is connected.

Projection into session time remains owned by scanner-core clock synchronization.

## Capability policy

This implementation does not advertise torch or device-certified Android depth support. Capability exposure remains gated by generated-project compilation and physical-device evidence.

## Verification boundary

Host CI syntax-checks the JNI C shim against the checked-in scanner-core header and continues to test scanner-core/Tauri Rust code. That does not prove Android NDK packaging, ARCore runtime behavior, camera ownership, or physical sensor correctness.
