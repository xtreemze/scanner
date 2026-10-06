# Guided capture onboarding

The onboarding experience is an operational calibration flow, not only a feature tour.

## Phase 1 — understand the available hardware

Show detected camera, motion, depth/LiDAR, ranging, flash, and HDR capabilities. Explain unavailable capabilities without blocking camera-only operation.

## Phase 2 — establish the environment

Guide the user to observe stable walls, floor, ceiling, corners, and large fixed surfaces. Display confidence and ask for explicit confirmation before promoting a stable surface into a locked structural constraint.

## Phase 3 — improve anchor geometry

Discover cooperating devices and recommend physical placement. Avoid collinear or tightly clustered anchors. Explain whether each peer is contributing ranging, visual observations, depth, illumination, or only coarse proximity.

## Phase 4 — establish spatial lock

Ask the user for the smallest set of movements/observations that reduces pose uncertainty. When confidence is weak, request reacquisition of a known structural reference instead of continuing to collect low-value object imagery.

## Phase 5 — scan

Show the live reconstruction and independent confidence overlays. Guidance should be spatially actionable: direction, distance, viewpoint, target region, or anchor action.

## Phase 6 — repair and refine

Let the user tap an uncertain region in the camera or model view. Convert the tap into a registered 3D repair target and guide the user until the relevant geometry, texture, or material confidence reaches the target threshold.

## Interaction requirements

- no hidden reliance on color alone for confidence;
- touch targets suitable for one-handed mobile use;
- reduced-motion mode;
- explicit permission rationale before requesting sensors;
- resumable setup after interruption;
- no implication that unsupported browser/PWA sensors are available merely because the native app supports them.
