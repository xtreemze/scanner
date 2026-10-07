# ADR 0010: appearance inference consumes explicit evidence

Status: accepted

## Decision

Appearance reconstruction is evidence-driven and remains separate from geometry authority.

Scanner represents:

- directional environment observations for progressive HDR coverage;
- controlled ambient/illumination frame pairs;
- capture stability constraints for those pairs;
- optional known emitter position in SessionWorld;
- PBR material estimates with explicit confidence and evidence identity.

A material estimate without provenance is invalid.

## Controlled illumination pairing

An ambient and illuminated observation may be treated as a controlled pair only when capture motion, time separation, and exposure mismatch remain within explicit thresholds.

The initial thresholds are policy defaults, not claims about laboratory calibration. Native adapters and later photometric solvers may provide better uncertainty models without changing the domain contract.

Known emitter pose is optional. When present, it is SessionWorld evidence and can support directional-light reasoning. When absent, downstream inference must not pretend the illumination direction is known.

## HDR coverage

Environment capture tracks directional coverage independently of image quality or material confidence. More images from the same direction do not falsely increase spherical coverage.

## PBR estimates

A PBR estimate contains:

- linear RGBA base color;
- metallic factor;
- roughness factor;
- confidence;
- evidence IDs.

All scalar values are validated to finite [0, 1] ranges.

The GLB exporter may project a validated PBR estimate into glTF metallic-roughness fields. Export does not make that estimate authoritative and does not increase its confidence.

## Non-goals of this slice

This does not implement:

- HDR image stitching or radiometric calibration;
- texture atlas projection;
- flash synchronization at platform level;
- inverse rendering;
- BRDF recovery;
- material inference from pixels.

Those remain later stages of issue #9 and require physical-device evidence.
