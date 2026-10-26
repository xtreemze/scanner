# Development

Scanner currently consists of a Vite/Lit PWA, a Tauri native host, and a Rust domain crate.

## Toolchain

- Node.js 24.21.0 LTS or newer compatible release
- pnpm 12.9.1
- Rust 1.99.0
- Tauri CLI 2.12.1

## Web/PWA

    corepack enable
    corepack prepare pnpm@12.9.1 --activate
    pnpm install --frozen-lockfile
    pnpm build
    pnpm dev

## Core domain

    cargo test --manifest-path crates/scanner-core/Cargo.toml --all-targets --locked

The Rust core owns capture/session contracts, structural state transitions, and deterministic guidance policy. UI and platform adapters consume these contracts but do not become parallel authorities.

## Tauri

Install the current Tauri platform prerequisites, then:

    pnpm tauri:dev

Lockfiles are committed for pnpm, scanner-core, and the Tauri host. CI requires frozen/locked resolution.

## Native sensor adapters

The active Tauri mobile plugin source lives under `plugins/scanner-sensors/`. The earlier `native/ios/` and `native/android/` files are retained only until the plugin integration is validated and will then be removed to avoid parallel adapter ownership.

The plugin Rust shell can be checked on the host with:

    cargo check --manifest-path plugins/scanner-sensors/Cargo.toml --all-targets

The iOS/Android native modules are still not device-certified.

The integration architecture is documented in `docs/decisions/0012-native-adapter-control-and-data-plane.md`: Tauri mobile plugins own low-rate control commands while camera/depth/IMU payloads should reach Rust through FFI/JNI rather than passing through the WebView.

Do not add a capability to `platform_capabilities` until the corresponding Tauri adapter path is compiled and operational for that session.

## Research-based core primitives

The core exposes `DepthFusionGate` to decide which retained depth frames are fresh, timestamp-consistent metric fusion candidates; use one gate per session and supply `DepthScaleEvidence` explicitly. It does not discard archived depth frames or substitute for pose, calibration, and uncertainty validation. `triangulate_session_rays` handles finite, forward two-view geometry **only after** calibration, feature correspondence, clock synchronization, and transforms into SessionWorld.

Both primitives have deterministic unit tests through the standard scanner-core `cargo test` command. The production reconstruction and peer pipelines still need to call them; see [the research plan](docs/research/2026-10-mobile-reconstruction.md) and [ADR 0016](docs/decisions/0016-independent-depth-and-multiview-geometry.md).

## Verification claims

A successful web build proves only type checking and Vite production bundling. Passing scanner-core tests proves only the deterministic Rust domain contracts covered by those tests. Tauri desktop builds, Android/iOS compilation, PWA installation, camera/IMU/depth capture, and physical multi-device calibration require separate evidence.
