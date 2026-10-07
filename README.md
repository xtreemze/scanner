# Scanner

Scanner is a real-time mobile spatial reconstruction system for progressively building, inspecting, correcting, and refining 3D models from camera, motion, depth/LiDAR, peer ranging, structural references, and guided user observations.

The product is designed around an active measurement loop rather than passive photogrammetry. Scanner can ask the user to move the camera, place an anchor, reacquire a wall, illuminate a region, or rescan a specific 3D area when that action is expected to reduce uncertainty.

## Product principles

- **See the reconstruction while capturing.** Geometry, coverage, confidence, texture, and later material estimates should improve progressively.
- **One SessionWorld.** No individual ARKit, ARCore, browser, or Tauri coordinate frame is authoritative.
- **Multi-device by design.** Phones may scan, anchor, observe, illuminate, or contribute ranging/depth.
- **Stable structure is explicit.** Walls, floors, ceilings, and other trusted surfaces can become user-confirmed constraints and can later be challenged by contradictory evidence.
- **Confidence stays visible.** High-quality textures must not conceal uncertain geometry or pose.
- **Raw evidence is preserved.** Derived meshes are replaceable projections, not the sole source of truth.
- **Native where necessary, web where useful.** The presentation/onboarding UI is a PWA and shared Tauri frontend; mobile sensors remain behind native adapters.

## Current stack

- Vite 8
- TypeScript 7
- Lit 3 web components
- vite-plugin-pwa 2
- Tauri 2.12
- Rust 1.99

The initial dependency baseline was selected from current stable releases at repository bootstrap.

## Repository layout

- `src/domain/` — cross-platform presentation-side contracts
- `src/platform/` — browser/Tauri adapter boundary
- `src/ui/` — reusable Lit presentation and onboarding components
- `crates/scanner-core/` — platform-independent Rust domain contracts
- `src-tauri/` — native host and future mobile platform adapters
- `docs/architecture/` — durable system contracts
- `docs/decisions/` — architecture decision records

## Architecture

The preferred ownership chain is:

    domain contracts
      -> deterministic planners / state machines
      -> adapters and services
      -> presentation models
      -> UI / platform projections

See [system architecture](docs/architecture/system.md), [guidance/confidence](docs/architecture/guidance.md), the [raw observation/domain authority ADR](docs/decisions/0003-raw-observations-and-domain-authority.md), and the [platform/device certification matrix](docs/verification/certification-matrix.md).

## Development

See [README_DEV.md](README_DEV.md).

## Status

The initial scaffold provides the PWA presentation/onboarding experience, Tauri host boundary, and first Rust domain contracts. Native camera/IMU/depth/LiDAR/UWB/Bluetooth ranging adapters and the actual reconstruction pipeline are intentionally tracked as subsequent implementation work rather than represented as complete.
