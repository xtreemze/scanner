# ADR 0011: progressive HDR and controlled material inference

Status: accepted

## Decision

Scanner's appearance pipeline remains evidence-driven and platform-independent.

### HDR environment

Environment observations are accumulated into directional radiance bins in SessionWorld. Each observation includes:

- source frame identity;
- direction;
- linear RGB evidence;
- relative exposure value;
- confidence.

Exposure-bracketed observations are normalized before fusion. Coverage and per-bin confidence remain visible so an incomplete environment map cannot be represented as complete.

The core can request the next missing environment direction. Platform UI may translate that direction into camera guidance.

### Controlled illumination

A controlled sequence starts with an ambient reference followed by deterministic flash-pulse steps from available peer emitters.

Emitter pose and calibration confidence remain explicit. Unknown emitter pose is valid evidence, but it cannot receive the same material-inference confidence as calibrated spatial illumination.

### Texture projection

Texture source selection is a deterministic scoring problem over:

- projected texel density;
- incidence angle;
- blur;
- exposure;
- pose confidence;
- occlusion.

A visually sharp image with severe occlusion must not outrank a slightly lower-resolution unobstructed view.

### Material inference

Ambient/illuminated regional statistics are reduced to separate:

- diffuse response evidence;
- specular-excess evidence;
- base-color evidence.

Approximate PBR inference produces base color, roughness, metallic and confidence. It does not claim physically exact BRDF recovery.

Metallicity is intentionally conservative and capped because ordinary phone flash observations are insufficient to strongly identify conductor behavior. Overall confidence is also capped when emitter position/calibration is weak.

The raw evidence IDs remain attached to the material estimate so future inverse-rendering implementations can replace the estimator without changing provenance.

## Platform boundary

iOS and Android adapters will implement exposure, flash/torch, camera synchronization, and peer triggering. They produce the observations consumed here; they do not own HDR/material state.
