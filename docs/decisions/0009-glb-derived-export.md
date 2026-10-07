# ADR 0009: GLB is a derived geometry export

Status: accepted

## Decision

Scanner may export the current reconstruction projection as glTF 2.0 binary (GLB), but the GLB is never authoritative session or resume state.

The exporter:
- consumes a validated `PreviewMesh` or a mesh extracted from `ReconstructionVolume`;
- writes metric positions as 32-bit floats;
- writes triangle indices as unsigned 32-bit integers;
- validates finite positions, triangle topology, and index bounds;
- emits correctly aligned GLB JSON/BIN chunks;
- preserves Scanner coordinates verbatim.

No implicit axis swap, handedness conversion, or unit scaling occurs inside serialization. If Scanner later adopts an explicit external-coordinate convention, that transform must be a reviewed contract rather than an exporter side effect.

## Current scope

The initial GLB path exports geometry only.

It does not yet claim:
- final-quality meshing;
- texture atlases;
- recovered PBR materials;
- normals/tangents;
- USDZ parity.

Those remain derived refinement/export stages and must not alter capture evidence, SessionWorld, checkpoints, or reconstruction authority.

## Rationale

Keeping export deterministic and dependency-free makes the geometry artifact testable before appearance reconstruction exists, while preserving the architectural distinction between resumable evidence and presentation/interchange outputs.
