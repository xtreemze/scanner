# ADR 0008: versioned session checkpoints

Status: accepted

A resumable scan uses a versioned checkpoint containing the canonical ScanSession, raw observation envelopes, SessionWorld state, and exact reconstruction fusion state.

Restore validates checkpoint and session schema versions plus session epochs. Invalid constraints, poses, voxel accumulators, capacity violations, duplicate state, and geometry evidence that references missing voxels are rejected.

Reconstruction resume restores weighted voxel accumulators rather than rebuilding from an exported mesh. This keeps resumed fusion equivalent to an uninterrupted scan.

Camera and depth assets referenced by observation asset IDs remain external storage payloads. The domain checkpoint stores their authoritative references and state, not duplicate binary assets.

Exported GLB, USDZ, images, and other presentation artifacts are derived outputs. They are not authoritative resume state.
