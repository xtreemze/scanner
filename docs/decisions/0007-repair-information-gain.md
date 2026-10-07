# ADR 0007: targeted repair accepts observations by information gain

Status: accepted

A repair target is a registered three-dimensional region with independent geometry, texture, and material needs.

Candidate observations are not accepted merely because they overlap the repair target. They must:

- fall inside the registered SessionWorld region;
- have valid confidence evidence;
- not duplicate an already accepted observation;
- increase one or more requested confidence dimensions by at least the configured minimum information gain.

Confidence combines incrementally so repeated useful evidence approaches certainty without exceeding one. Unrequested dimensions are left unchanged.

Repair status exposes:

- independent geometry, texture, and material confidence;
- configured target confidence;
- normalized progress across requested dimensions;
- completion state;
- accepted observation count.

This status is a presentation model for overlays and guidance. It does not replace raw observation provenance or the canonical reconstructed geometry.
