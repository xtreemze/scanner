# Mobile reconstruction research transfer — 26 October 2026

## Scope and evidence standard

This note translates computer-vision research into implementation decisions for Scanner across **Android, iOS, and WebGPU/PWA**, with equal attention to each target. It is not an assertion that the methods are integrated or device-certified. Research authors' measurements are not Scanner performance measurements. Distinguish four separate workloads: camera/IMU acquisition and pose tracking, depth integration and mesh extraction, reconstruction optimization, and rendering.

### Primary research

| Work | Primary paper | Directly evidenced | Not demonstrated for Scanner |
| --- | --- | --- | --- |
| Mobile3DScanner (ISMAR 2021) | [paper](https://www.cad.zju.edu.cn/home/gfzhang/papers/mobile3dscanner/mobile3dscanner_ismar2021.pdf) | iPad Pro 2020 RGB-D / VI-ICP tracking, keyframe TSDF fusion, adaptive voxel resizing and post-capture optimization | Scanner native adapter and TSDF performance |
| MobileFusion (TVCG 2015) | [paper](https://ori.ox.ac.uk/media/5802/2015ismar_ondruska.pdf) | Author-reported on-phone 25 Hz live surface modeling using a CPU/GPU pipeline; historical hardware | Performance on current Android, iOS or WebGPU |
| PocketGS (2026 preprint) | [paper](https://arxiv.org/abs/2601.17354) | Authors report on-device Gaussian training on an iPhone 15, about four minutes with roughly 2.2 GB average peak training memory | Continuous camera-to-mesh processing at 30/60 FPS, production-quality metric meshes |
| Mobile-GS (2026) | [paper](https://arxiv.org/abs/2603.11531) | Author-reported mobile Gaussian **rendering**, including 127 FPS initially and 74 FPS thermally stabilized on Snapdragon 8 Gen 3 | Training on that Snapdragon, sustained live capture-to-reconstruction |
| MetaDepth-CPU (CVPRW 2026) | [paper](https://openaccess.thecvf.com/content/CVPR2026W/ECV/html/Mapeke_MetaDepth-CPU_Zero-Shot_Monocular_Depth_Estimation_for_Edge_Devices_CVPRW_2026_paper.html) | Mobile/edge CPU monocular depth inference | Calibrated metric scale or complete SLAM pipeline |
| VGGT (CVPR 2025) | [paper](https://openaccess.thecvf.com/content/CVPR2025/html/Wang_VGGT_Visual_Geometry_Grounded_Transformer_CVPR_2025_paper.html) | Joint inference of visual geometry, cameras and point maps on research hardware | Viable continuous on-phone capture pipeline |
| VIGS-SLAM (ECCV 2026) | [project/paper](https://vigs-slam.github.io/) | Visual-inertial constraints and pose-consistent neural mapping | Reproducible cross-platform phone compute/memory envelope |
| MNE-SLAM (CVPR 2025) | [paper](https://openaccess.thecvf.com/content/CVPR2025/html/Deng_MNE-SLAM_Multi-Agent_Neural_SLAM_for_Mobile_Robots_CVPR_2025_paper.html) | Distributed submap / loop-closure concepts | Physical heterogeneous phone certification |
| OminiMAG-SLAM (CVPR Findings 2026) | [paper](https://openaccess.thecvf.com/content/CVPR2026F/html/Ding_OminiMAG-SLAM__Unified_Online_Dual_Graph_Optimization_for_Multi-Agent_Gaussian_CVPRF_2026_paper.html) | Local/global graphs for multi-agent Gaussian SLAM | Real-world phone telemetry and bandwidth limits |
| Smartphone synchronization (ICET 2026) | [paper](https://doi.org/10.1109/ICET69987.2026.11659286) | Offline unsynchronized-smartphone reconstruction with an audio timing cue | Low-latency synchronized capture and live triangulation |

Do not compare author's FPS figures across papers without matching task, input resolution, device, scene, stabilization period and metric. Papers are references, not dependencies.

## Decisions now

1. **Canonical geometry remains metric, not Gaussian.** The current `SparseSurfaceVolume` is a weighted occupied-voxel *point/surface preview* with cube-face mesh extraction, **not a TSDF**. Its existing provenance/confidence and object-local frame are worth retaining.
2. **Gate independent metric depth before fusion, not before archival.** New `fusion_admission.rs` rejects reprojected frames, same/out-of-order timestamps, uncalibrated relative depth, excess timestamp uncertainty, clock-domain switches within an epoch, and stale epochs. Per-device state is bounded; tests cover these distinctions. Platform-derived raw evidence remains in the capture ledger. A future fusion worker must explicitly call the gate.
3. **Triangulation requires SessionWorld alignment and quality checks.** New `triangulation.rs` computes the closest point between two forward rays only with sufficient parallax, acceptable ray miss and finite positive range. It **does not** perform feature matching, synchronization, transform estimation or covariance propagation; callers must establish these.
4. **Native data path remains outside the WebView.** Preserve ARKit/ARCore -> Swift/Kotlin -> FFI/JNI -> Rust for camera, depth and IMU. WebGPU and Web Workers may render or handle bounded computational projections, but should not serialize full-rate buffers through Lit.
5. **Adapt depth confidence to actual sensor provenance.** ARCore reprojected raw depth is not new independent evidence. Retain ARKit scene-depth confidence. Software relative depth needs explicit scale calibration (and geometric evaluation) before metric integration; do not claim that a learned estimate is a LiDAR-quality observation.
6. **Future volumetric pipeline:** chunked sparse TSDF with truncation and bounded memory; dirty-chunk incremental isosurface extraction; keyframe-associated integration logs / chunk provenance; loop-closure reintegration or transformable submaps; eviction/checkpointing. Never silently change canonical object identity when an AR session relocalizes.
7. **Neural work remains optional and isolated.** Use off-critical-path Gaussian optimization or compact rendering as a replaceable appearance projection only after device benchmarks pass.

## Follow-up engineering and acceptance

### A. First native depth-to-mesh end-to-end path (P0)

- Reconstruct calibrated depth pixels into rays with validated intrinsics, extrinsics, metric units, stride/confidence handling and invalid-pixel masks.
- Use `DepthFusionGate` before integration. Explicitly classify raw platform depth versus separately scale-aligned software depth.
- Fuse on a bounded worker lane, not the UI thread. Start with the existing sparse surface projection before replacing/augmenting it with true TSDF.
- Verify real camera movement, offline capture replay, pose corrections, repeat-frame deduplication, memory bounds and GLB metric output.
- Do not report device verification from host Rust tests or Android APK assembly alone.

### B. Multi-peer triangulation and SessionWorld constraints (P1)

- Synchronize monotonic clocks with uncertainty (existing `clock.rs`), and align device-local camera transforms to SessionWorld using supported constraints.
- Match the **same feature** observed from two or more calibrated devices; use RANSAC / outlier rejection, reprojection residual and measurement covariance.
- Pass rays to `triangulate_session_rays` only after alignment and time checks. Its result is a **candidate** geometry measurement and does not automatically become a trusted landmark.
- Reject weak parallax, dynamic mismatches, clock drift, lost peers and range-only ambiguity; compare two-device and non-collinear three-device topologies.

### C. Performance certification (P0/P1)

Record scene name, commit, exact device, OS, AR framework/WebView/browser, sensor tier, source resolution, sustained capture duration, thermal state and battery impact. Use camera-only, software depth, physical LiDAR/ToF and 2-/3-peer fixtures. Capture:

- camera/IMU rates, fresh depth rate and admitted-fusion rate independently;
- median/P95 capture-to-preview and mesh update latency; dropped/backlogged frames;
- CPU/GPU frame time, memory peak, energy and ten-minute sustained performance;
- ATE/RPE where available; point/mesh distance to metric reference; completeness;
- peer clock offset/drift uncertainty, reprojection residual, relative-pose error and communication bytes/sec;
- relocalization, restart/resume, peer drop/rejoin and bad-constraint rejection.

Initial *targets*, not achieved claims: 60 FPS presentation where supported; >=30 Hz pose updates where supported; adaptive depth integration based on fresh evidence; no main-thread sensor buffer transport. Thermal or accuracy regressions should make the capability non-certified on that exact device.

## Research maturity labels

- **Established:** native visual-inertial tracking, calibrated multiview geometry, robust correspondence rejection, TSDF/RGB-D integration, incremental mesh extraction.
- **Mobile validated in author experiments, not Scanner:** incremental RGB-D reconstructions, edge monocular depth inference, Gaussian rendering, bounded on-device Gaussian refinement.
- **Experimental for Scanner:** full neural visual-inertial SLAM, heterogeneous multi-phone live neural map fusion, dynamic 4D reconstruction and photometric material recovery.

Related documents: [system architecture](../architecture/system.md), [sensor data-plane decision](../decisions/0012-native-adapter-control-and-data-plane.md), [certification matrix](../verification/certification-matrix.md).
