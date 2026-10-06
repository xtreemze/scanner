# ADR 0004: SessionWorld uses a robust spatial constraint graph

Status: accepted

## Decision

Multi-device spatial state is represented as weighted constraints in scanner-core.

A constraint carries:
- stable identity;
- session epoch;
- source/provenance;
- uncertainty;
- typed measurement payload.

The first solver stage is deterministic propagation, not a full nonlinear optimizer. Absolute poses can seed SessionWorld, relative poses can propagate from known devices, and directed range observations can seed a peer when distance and bearing are both observable.

Undirected range alone must not invent an unobservable position.

## Robustness

When both device transforms are known, range and relative-pose constraints are evaluated against the current world state. Large residuals are downweighted with a robust weight instead of directly moving devices.

This prevents a transient bad Bluetooth/UWB/visual measurement from teleporting the reconstruction.

## Future optimizer

A later nonlinear least-squares/factor-graph implementation may optimize all transforms jointly. It must preserve:
- constraint identity and provenance;
- uncertainty;
- epoch semantics;
- residual diagnostics;
- the rule that device-local platform frames do not become SessionWorld authority.

Gravity and structural-plane constraints are represented now so they can participate in the future optimizer without changing the public domain contract.
