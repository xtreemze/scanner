# System architecture

Scanner treats reconstruction as an active measurement system.

## Canonical ownership

SessionWorld is the canonical spatial frame for a scan session. Device-local AR coordinate systems are observations/adapters and may be re-estimated without changing object identity.

The canonical data flow is:

    raw observations
      -> validated observation contracts
      -> spatial constraint graph
      -> deterministic planners / estimators
      -> canonical session state
      -> reconstruction projections
      -> presentation models
      -> PWA / Tauri / native UI

## Raw observations

A saved session should preserve enough source evidence to re-run improved reconstruction algorithms:

- timestamped calibrated camera observations;
- intrinsics/extrinsics and exposure metadata;
- IMU samples;
- depth/LiDAR and confidence when available;
- peer ranging and its uncertainty;
- device pose estimates and their provenance;
- controlled illumination/flash events;
- user-confirmed correspondences and structural assertions.

Derived meshes and textures are not the only source of truth.

## Spatial constraint graph

Every sensor contribution is a weighted constraint with provenance and uncertainty. Examples include visual-inertial pose deltas, UWB range/direction, Bluetooth channel-sounding constraints, visual peer transforms, gravity, structural planes, and manually confirmed landmarks.

BLE RSSI may contribute discovery/proximity information but must not be treated as centimeter-precision geometry.

## Structural surfaces

Floors, walls, ceilings, and stable planar surfaces are first-class entities. Their lifecycle is:

candidate -> stable -> suggested-lock -> user-confirmed -> locked -> challenged

Locked means strong constraint, not infallible geometry. Contradictory observations must be retained and can challenge a locked surface.

## Reconstruction

The live path and refinement path are distinct:

- live: bounded-latency point/depth integration, confidence updates, progressive surface representation, incremental mesh extraction;
- refinement: higher-cost geometry, texture, lighting, and material optimization;
- experimental: neural/3D Gaussian representations may supplement but do not replace canonical metric geometry.

## Multi-device

A peer may be a scanner, anchor, observer, or illuminator. The protocol should generalize beyond phones to tablets, headsets, fixed cameras, and UWB anchors.

Peers never own SessionWorld. They contribute observations to it.

## Targeted repair

A user tap is raycast into canonical geometry and converted into a 3D repair region. New observations are accepted based on whether they reduce uncertainty for that region, not merely because they overlap the screen-space tap.

## Appearance

HDR environment capture, ambient/flash pairs, and multi-device controlled illumination provide evidence for texture and approximate PBR material inference. Material estimates always carry confidence.
