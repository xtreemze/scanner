# ADR 0006: object-local reconstruction and bounded evidence links

Status: accepted

Scanner supports two reconstruction frames: SessionWorld for fixed environment geometry and ObjectLocal for movable object geometry.

Object-local geometry is fused in the object's own coordinates. The object-to-session pose is a projection used for display, raycasts, and interaction. Changing that pose must not rewrite or reintegrate the object's stored surface geometry.

ReconstructionVolume also records bounded per-voxel evidence links: recent unique observation IDs, contributing device IDs, and first/last projected session timestamps. Full raw capture records remain authoritative; voxel evidence is only an index back to those records.

The existing SessionWorld SparseSurfaceVolume API remains unchanged for compatibility. ReconstructionVolume wraps it with explicit frame and evidence behavior.

Points, preview meshes, and raycast hits returned by ReconstructionVolume are projected into SessionWorld for consumers even when the canonical geometry is stored object-local.
