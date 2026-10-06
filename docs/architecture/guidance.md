# Guidance and confidence

Scanner can ask the user to physically improve the measurement.

## Confidence fields

At minimum track independent confidence for:

- geometry;
- pose;
- surface coverage;
- view-angle diversity;
- texture;
- material/illumination;
- structural constraints;
- peer/anchor geometry.

Do not collapse these into one number in the canonical model.

## Measurement actions

The guidance planner may request:

- move/rotate the scanner;
- change capture distance;
- observe a surface, edge, or corner;
- move/add an anchor;
- hold still for synchronization;
- confirm or lock a structural surface;
- reacquire a known reference;
- rescan a 3D region;
- remove a temporary occluder;
- perform a controlled illumination observation;
- capture missing HDR environment coverage.

The initial planner may be deterministic. Recommendations should optimize expected uncertainty reduction against user effort and acquisition time.

## User assertions

User input can provide high-confidence semantic/correspondence evidence, but must not silently replace sensor geometry. For example, “this is a wall” is semantic evidence while its plane parameters remain measured.
