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

Platform acquisition source baselines live under `native/ios/` and `native/android/`. They are not yet wired into generated Tauri mobile projects and are not device-certified.

The integration architecture is documented in `docs/decisions/0012-native-adapter-control-and-data-plane.md`: Tauri mobile plugins own low-rate control commands while camera/depth/IMU payloads should reach Rust through FFI/JNI rather than passing through the WebView.

Do not add a capability to `platform_capabilities` until the corresponding Tauri adapter path is compiled and operational for that session.

## Verification claims

A successful web build proves only type checking and Vite production bundling. Passing scanner-core tests proves only the deterministic Rust domain contracts covered by those tests. Tauri desktop builds, Android/iOS compilation, PWA installation, camera/IMU/depth capture, and physical multi-device calibration require separate evidence.
