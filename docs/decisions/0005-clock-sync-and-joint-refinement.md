# ADR 0005: device clocks are modeled explicitly and SessionWorld refinement is robust and bounded

Status: accepted

## Clock synchronization

Each cooperating device may run on a different monotonic clock. Scanner therefore models device-to-session time as:

session_time = device_time + offset + drift * (device_time - reference_time)

Clock exchange samples use four timestamps so round-trip delay can be estimated. The synchronizer:

- rejects invalid timestamp ordering;
- estimates offset and linear drift from multiple exchanges;
- weights lower-round-trip exchanges more strongly;
- exposes residual/network-delay uncertainty;
- projects device-local timestamps into session time without rewriting the original observation timestamp.

Raw source timestamps remain provenance. The projected session time is derived state.

## Joint spatial refinement

The first joint refinement stage optimizes device translation using existing SessionWorld range and relative-pose constraints.

Absolute SessionWorld pose constraints remain fixed anchors.

Each iteration:
- evaluates residuals against the current device poses;
- weights residuals by measurement uncertainty and a robust loss;
- accumulates corrections across all connected devices;
- clamps each device step to a configured maximum;
- stops when corrections converge or the iteration budget is exhausted.

This is intentionally dependency-free and bounded. It is a real nonlinear correction loop because range residuals depend on current Euclidean distance, but it is not yet a general bundle-adjustment or six-degree-of-freedom solver.

## Safety and continuity

Bad measurements must reduce influence rather than cause discontinuous pose jumps.

Scanner handoff does not create a new SessionWorld epoch or redefine existing device/object identity.

A later solver may refine orientations and structural/gravity factors jointly, but it must preserve the same constraint identity, provenance, uncertainty, clock, residual-diagnostic, and epoch contracts.
