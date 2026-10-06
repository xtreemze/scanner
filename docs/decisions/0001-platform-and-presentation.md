# ADR 0001: PWA presentation and Tauri native host

Status: accepted

## Decision

Use one Vite/Lit frontend as:

- the GitHub Pages presentation site;
- installable PWA;
- onboarding experience;
- shared presentation layer inside Tauri.

Use Tauri 2 as a native host and permission boundary. Native mobile sensor access will be implemented through focused platform adapters/plugins rather than by placing reconstruction authority in the webview.

Keep reconstruction/domain logic in Rust crates that do not depend on Tauri.

## Rationale

This preserves an immediately accessible browser experience while enabling native iOS/Android APIs where Web APIs are insufficient. Lit keeps reusable UI surfaces standards-based and embeddable.

## Consequences

The web/PWA tier may have fewer sensor capabilities than native builds. Feature detection and explicit capability contracts are required. UI code must tolerate missing depth, ranging, flash control, and other native-only capabilities.
