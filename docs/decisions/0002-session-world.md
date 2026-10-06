# ADR 0002: SessionWorld is the canonical spatial frame

Status: accepted

## Decision

No device-local AR origin is canonical. Every device has a time-varying transform into a session-owned SessionWorld.

Structural references, reconstructed objects, peer poses, and repair regions are represented in this session frame or in explicit object-local frames derived from it.

## Rationale

This avoids locking the architecture to the first scanner and enables relocalization, multiple anchors, scanner handoff, distributed observation, and later fixed spatial infrastructure.

## Invariant

A platform callback may update constraints, but it may not directly mutate canonical object geometry or overwrite the session transform without going through the spatial solver/state transition.
