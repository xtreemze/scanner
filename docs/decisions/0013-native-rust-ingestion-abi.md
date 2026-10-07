# ADR 0013: scanner-core owns the native ingestion ABI

Status: accepted

## Decision

The high-rate mobile data plane enters Rust through a platform-neutral C ABI exported by `scanner-core`.

The initial ABI supports:

- per-device, per-epoch opaque ingestion sessions;
- camera frame metadata;
- IMU samples;
- depth/confidence buffers;
- bounded ingestion diagnostics.

Tauri is not part of this ABI. Swift and Android JNI wrappers call it directly.

## Ownership

Native platforms borrow buffers only for the duration of an ingestion call. Rust validates the descriptor and copies accepted depth/confidence evidence into a bounded Rust-owned queue before returning.

This establishes explicit lifetime ownership and avoids:

- native -> WebView -> Rust payload round trips;
- dangling platform buffer pointers;
- unbounded accumulation when reconstruction cannot keep up.

The queue currently retains at most four depth frames and 256 canonical observations. Oldest evidence is dropped under pressure and the drop count is observable.

## Canonical observations

Camera metadata, IMU, and depth descriptors become existing versioned `RawObservation` values. Platform DTOs do not become another domain model.

Device-local camera pose remains device-local evidence. The ingestion ABI does not label it as SessionWorld pose.

Native timestamps retain their declared clock domain and uncertainty. Clock projection into SessionWorld time remains owned by the Scanner clock model.

## Buffer formats

The first depth formats are:

- unsigned 16-bit millimetres, matching ARCore raw-depth representation;
- 32-bit float metres, matching iOS depth-map representation.

Row stride is explicit. Optional confidence is an 8-bit-per-pixel plane with its own row stride.

## ABI stability

All boundary structs use `repr(C)` scalar fields. Raw enum-like inputs use integer discriminants and are validated before entering the Rust domain.

The checked-in `native/scanner_core.h` is the native contract consumed by Swift/C and by the future Android JNI shim.

## Follow-up

The next platform slices should:

1. link scanner-core into generated Tauri iOS/Android projects;
2. call the C ABI from Swift;
3. implement a thin JNI shim for Kotlin/direct ByteBuffer access;
4. connect accepted Rust-owned evidence to clock projection, SessionWorld, and reconstruction consumers;
5. add physical-device evidence before closing issues #2 and #3.
