# ADR 0016: independent depth admission and guarded multiview geometry

Status: **accepted for deterministic core boundaries; device/reconstruction wiring pending**

## Context

Native AR pose/depth feeds provide samples faster than each new independent depth measurement. ARCore raw depth can return temporally reprojected pixels. Depth from monocular learned models may be relative-scale, not metric. Two phone cameras do not provide a trusted shared frame, correspondence or timestamp merely by joining the same session. Scanner preserves raw observations, while fusion and constraint optimization must reject misleading independent evidence.

## Decision

1. Preserve all capture evidence and timestamps at the canonical ingestion boundary. **Never discard an observation solely because it is ineligible for metric fusion.**
2. Before a reconstruction worker integrates depth, it must call the per-session `DepthFusionGate` in `scanner-core`. Pass honest `DepthScaleEvidence` and new-versus-reprojected frame metadata. The gate enforces a per-device epoch/clock-domain timestamp watermark and bounded device registry. Its default maximum input timestamp uncertainty (50 ms) is a policy starting point, not a claim of sufficient inter-device synchronization.
3. For software monocular depth, `UncalibratedRelative` is ineligible for metric fusion; independent scale calibration and an adapter guarantee are required before `AlignedSoftwareMetric` is passed. The gate does **not** establish that calibration was correct.
4. Use `triangulate_session_rays` only for calibrated, synchronized, correctly associated camera rays **already expressed in SessionWorld**. A triangulated point remains a candidate until downstream robust matching, uncertainty propagation and a constraint admission check accept it.
5. Neither guard is an implementation of real-time TSDF, visual feature matching, VIO, peer registration, scene meshing or sensor calibration. Those require separate implementations and physical evidence.
6. The next geometry pipeline is spatially chunked and bounded. Association between source keyframes and dirty chunks supports relocalization/loop-closure correction by submap adjustment or replay/reintegration rather than uncontrolled model warping.
7. Keep neural models as independent, versioned optional projections. Neither their output nor perceptual quality silently overrides canonical metric observations.

## Consequences and verification

- Callers that replay recordings should create a new gate per replay/session or reset the appropriate peer. A new epoch resets one device's watermark; an old epoch does not regress it.
- Do not mix clock domains within one device epoch; map timestamps first with `SessionClockSynchronizer`, or reset to a new session epoch when an adapter's clock authority changes.
- Unlike source retention, fusion admission is deliberately stateful. The gate should be called when a validated frame is ready to enter fusion, not during generic observation storage.
- Unit tests cover rejection categories; real device tests must measure depth freshness, accepted frame counts, triangulation reprojection residuals, memory/latency and thermal behavior.
- Physical Android, iOS and browser certification remains required under [the matrix](../verification/certification-matrix.md). No CI success certifies a hardware sensor.

See [research evidence and proposed sequence](../research/2026-10-mobile-reconstruction.md).
