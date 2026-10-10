# Certification matrix

This document defines the minimum evidence required before Scanner can claim a capability is supported, verified, or release-ready on a platform.

A build, simulator run, unit test, browser feature check, and physical-device verification are different evidence classes. Do not substitute one for another.

## Status vocabulary

- **Contracted** — domain/API contract exists and is covered by deterministic tests.
- **Build-verified** — the relevant target compiles/builds successfully in the stated environment.
- **Browser-verified** — behavior was exercised in the named browser/version.
- **Device-verified** — behavior was exercised on named physical hardware/OS.
- **Multi-device-verified** — behavior was exercised across the stated physical peer topology.
- **Not yet certified** — no current evidence meets the required bar.

Every certification record must include:
- commit SHA or release version;
- exact device model;
- OS version;
- browser/WebView/Tauri version as applicable;
- relevant sensor capabilities;
- test procedure;
- observed result;
- known limitations/regressions.

## Matrix

| Surface / capability | Current evidence | Minimum certification evidence |
| --- | --- | --- |
| GitHub Pages presentation | CI build + Pages deploy | Chrome current stable and Beta browser verification, navigation, layout, accessibility smoke test |
| PWA install | build artifact only | Physical Android install from deployed origin; launch standalone; offline shell after first load; update cycle |
| PWA offline shell | service worker generated | Browser/device verification with network disabled after install |
| Browser camera fallback | capability detection only | Physical mobile browser permission flow, capture start/stop, orientation change, denied/revoked permission cases |
| Browser IMU fallback | capability detection only | Physical mobile browser with motion permission where applicable; timestamps and orientation/motion sanity checks |
| Tauri desktop host | scaffold only | macOS/Windows/Linux build evidence separately; startup and IPC capability query |
| Tauri Android | native plugin + ARCore/JNI ingestion + native frame-pump source; host CI only | Android build + physical-device launch; native plugin permission, ARCore frame-pump, JNI symbol-resolution, and lifecycle evidence |
| Tauri iOS | scaffold only | iOS build + physical-device launch; native plugin permission and lifecycle evidence |
| Camera-only scanning | domain contracts only | Physical device with no hardware depth; live pose/capture/reconstruction session |
| Software-depth scanning | domain contracts only | Supported physical ARCore/ARKit device; depth/confidence ingestion and reconstruction evidence |
| Hardware depth / LiDAR | domain contracts only | Physical LiDAR/ToF device; synchronized camera/depth/pose evidence and confidence propagation |
| Session checkpoints/resume | unit-tested core | Device/app interruption and restart with persisted assets; resumed fusion compared with uninterrupted control |
| GLB geometry export | unit-tested core | Export from representative scan; validate with independent glTF validator/viewer; metric dimensions checked |
| USDZ export | not implemented | Physical Apple workflow; independent USDZ/Quick Look validation |
| Structural locking/relocalization | unit-tested domain | Physical room scan; lock surfaces; lose/reacquire tracking; demonstrate bounded drift against references |
| Two-device SessionWorld | unit-tested solver | Two physical peers with synchronized clocks and at least two independent spatial constraints |
| Three-device anchor topology | domain/solver only | Three physical peers; non-collinear placement; anchor removal/rejoin; residual/confidence evidence |
| UWB/ranging | domain contracts only | Physical compatible devices; range/direction samples, uncertainty, obstruction/outlier tests |
| Bluetooth ranging/channel sounding | domain contracts only | Compatible physical devices/API; measured uncertainty under orientation, body occlusion, and multipath |
| BLE RSSI proximity | architectural fallback only | Verify it remains coarse proximity evidence and never seeds precision position |
| Targeted repair | unit-tested core | Physical scan; camera tap/raycast; move device; confirm repair target remains registered and confidence improves |
| HDR environment capture | not implemented | Physical device; exposure-bracket/environment coverage and reconstructed HDR map inspection |
| Controlled flash capture | not implemented | Physical device; ambient/flash pairs with synchronized pose/exposure metadata |
| Multi-device illumination | not implemented | At least two physical peers; controlled emitter/camera timing and known relative pose |
| PBR material estimation | not implemented | Reference materials with known qualitative properties; repeatability and confidence/error reporting |
| Accessibility | basic semantic implementation | Keyboard/focus, screen reader/accessibility tree, zoom/reflow, reduced motion, touch target checks on deployed UI |

## Required device tiers

Maintain evidence across these tiers rather than treating one flagship phone as representative:

1. **Camera-only** — no usable hardware depth.
2. **Software-depth** — AR framework depth without dedicated LiDAR/ToF.
3. **Hardware-depth** — LiDAR or ToF/depth sensor.
4. **Ranging-capable** — UWB or platform channel-sounding support.
5. **Cooperating peer** — second/third device participating as anchor, observer, or illuminator.

## Multi-device test scenes

Use at least three repeatable physical scenes:

- **Planar room** — floor plus two or more walls/corners for structural constraints.
- **Small object** — object with concavities, occlusion, varied texture, and repair targets.
- **Reflective/material scene** — matte, glossy, metallic, and translucent/ambiguous regions to prevent material confidence from being inferred from appearance alone.

## Claim rules

A successful Linux CI run does not certify Tauri desktop generally.

A simulator does not certify a physical sensor.

A JavaScript API existing on `window` does not certify hardware availability or permission.

An ARKit/ARCore pose does not itself certify Scanner's SessionWorld solver.

A visually plausible model does not certify geometry accuracy.

A successful GLB parse does not certify material fidelity.

Any public README/site claim should be no broader than the strongest evidence recorded here.
