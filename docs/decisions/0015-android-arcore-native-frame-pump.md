# ADR 0015: Android ARCore native frame pump

Status: accepted

## Decision

The Android scanner-sensors plugin owns the ARCore `Session.update()` cadence for the current native capture path.

The frame pump:

- runs on the Android main thread through `Choreographer`;
- configures `Config.UpdateMode.LATEST_CAMERA_IMAGE` so update calls do not block the UI waiting for a camera frame;
- ignores repeated frame timestamps;
- forwards fresh `Frame` instances directly to the existing Kotlin -> JNI -> scanner-core ingestion path;
- does not expose frame payloads to the WebView;
- does not own SessionWorld, reconstruction, observation identity, or confidence state.

## Camera texture mode

The plugin configures `Config.TextureUpdateMode.EXPOSE_HARDWARE_BUFFER` for the native frame pump.

This avoids requiring the Tauri WebView or a second renderer to create and own an OpenGL camera texture merely to drive ARCore updates. The current path therefore requires Android API 27 or newer.

On lower API levels, Scanner must not advertise platform tracking through this plugin until a renderer-owned `GL_TEXTURE_EXTERNAL_OES` integration is implemented and verified.

## Lifecycle

Explicit session start:

1. verifies camera permission;
2. requests/validates ARCore installation;
3. creates or reuses the ARCore session;
4. creates one scanner-core ingestion session;
5. configures depth and hardware-buffer update mode;
6. resumes ARCore;
7. starts the native frame pump and IMU acquisition.

Explicit stop releases the frame pump, IMU listeners, ARCore session, and Rust ingestion handle.

Activity pause stops the frame pump before pausing ARCore. Activity resume restarts the existing native data path without creating a second canonical state owner.

## 16 KB native pages

Scanner's Android Rust targets and JNI shim explicitly request 16 KB ELF segment alignment. Host CI verifies the JNI linker recipe produces 0x4000 LOAD alignment.

This is build-recipe evidence only. Android package alignment, prebuilt dependency compatibility, and runtime behavior still require a generated Android build and physical-device evidence.

## Certification boundary

This ADR does not certify Android support. Issue #3 remains open until generated Tauri Android builds, ABI symbol resolution, 16 KB packaging, and physical camera/depth tests are recorded in the certification matrix.
