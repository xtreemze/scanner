# ADR 0003: raw observations and Rust domain authority

Status: accepted

## Decision

The Rust scanner-core crate owns the canonical capture/session contracts and deterministic state transitions.

Raw observations are immutable evidence envelopes with explicit:

- observation identity;
- device identity;
- session epoch;
- timestamp and clock domain;
- timestamp uncertainty;
- source/provenance;
- typed payload.

Platform adapters may translate ARKit, ARCore, camera, IMU, ranging, or user input into these envelopes. They may not redefine the domain contract.

## Epoch semantics

A session epoch is a generation boundary. Observations from another epoch are rejected rather than silently merged.

Out-of-order observations within the same epoch may be accepted because sensor and transport pipelines can legitimately reorder delivery. The observation ledger preserves a monotonic arrival watermark without rewriting source timestamps.

Duplicate observation identity is rejected.

## Structural surfaces

Structural-surface geometry is measured evidence. User confirmation changes constraint state, not measured plane parameters.

The supported state progression is:

candidate -> stable -> suggested-lock -> user-confirmed -> locked -> challenged

A challenged surface retains its original evidence and may be revalidated or explicitly unlocked.

## Guidance

The first guidance planner is deterministic. It prioritizes recovery of global pose/structure before collecting more object detail, then peer/anchor geometry, geometry/viewpoint diversity, coverage, appearance, and environment acquisition.

Later optimization or learned policies may replace scoring implementations without changing the MeasurementAction domain contract.
