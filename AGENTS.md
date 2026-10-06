# Repository execution guidance

## Authority

Use repository truth in this order:

1. this file and task-specific guidance;
2. source, tests, manifests, lockfiles, schemas, and CI;
3. README_DEV.md;
4. docs/architecture/ and docs/decisions/;
5. current GitHub issues;
6. current pull requests;
7. historical notes.

## Architecture

Prefer:

domain contracts -> deterministic planners/state machines -> adapters/services -> presentation models -> UI/platform projections

A platform adapter, Tauri command, browser component, cache, telemetry stream, or renderer must not become a second authority for scan/session state.

The canonical coordinate frame is SessionWorld, never an ARKit/ARCore/WebXR/Tauri-local origin.

Raw observations remain provenance. Derived point clouds, meshes, textures, material estimates, confidence maps, and presentation state are replaceable projections.

Platform-specific APIs belong behind adapters. Cross-platform domain contracts must not depend on Swift, Kotlin, Tauri, Lit, ARKit, ARCore, or vendor DTOs.

## Interaction and accessibility

Prefer semantic HTML, modern CSS, and native platform/browser APIs before dependencies. Pointer, touch, keyboard, reduced-motion, focus, zoom/reflow, and screen-reader behavior are first-class requirements.

## Verification

Record exactly what was run and on which candidate. Do not describe unrun device, browser, mobile, sensor, or native checks as verified.
