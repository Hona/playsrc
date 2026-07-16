# Physics Body And Constraint Family Inventory

Owner: [`../ROADMAP.md`](../ROADMAP.md)

State: Candidate; Not accepted.

| Field | Current value |
|---|---|
| Authority identity | Valve Source SDK 2013 VPhysics environment, body, relationship, constraint, spring, controller, vehicle, fluid, ragdoll, bone-follower, prop, and integration declarations; current playsrc Owner Registry; exact target-game occurrence registries. |
| Authority revision | Source SDK commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`; current playsrc Owner Registry; accepted TF2, CS:S, and legacy Source 1 CS:GO occurrence registries Missing. |
| Generator command | Missing. The future command is owned by `tools/playsrc`. |
| Output path | `packages/runtime/physics/inventories/body-and-constraint-families.md` |
| Item count | 30 candidate items; 0 accepted items. |

Every item is owned by Physics. `Required seam` identifies a producer or consumer and never transfers physical-state, persistent-contact, or solver ownership. `Unsupported` is the current playsrc coverage classification; it does not assert inventory acceptance.

## Body Motion Classes

| Stable identity | Physics-owned observable contract | Required seam | Current coverage |
|---|---|---|---|
| `body.motion.static` | Retain an infinite-mass, nonintegrated transform that participates in collision, contacts, constraints, triggers, and fluids without accepting force-driven displacement. | Map, Collision, or Entity supplies shape, transform, material, contents, and owner identity. | Unsupported |
| `body.motion.kinematic` | Follow one supplied transform per tick, derive linear and angular surface velocity, and impart physical contact response without accepting force-driven displacement. | Entity, Movement, StudioModel, or a game supplies the authoritative target transform. | Unsupported |
| `body.motion.motion-disabled` | Retain a dynamic body's mass, inertia, velocities, contacts, and relationships while pinning force-driven motion; reenabling motion restores dynamic behavior without identity replacement. | Entity or game supplies enable/disable commands. | Unsupported |
| `body.motion.dynamic` | Integrate finite-mass linear and angular state under gravity, damping, drag, impulses, contacts, constraints, controllers, and fluids. | Simulation supplies the fixed tick and ordered commands. | Unsupported |

## Body Shape Families

| Stable identity | Physics-owned observable contract | Required seam | Current coverage |
|---|---|---|---|
| `body.shape.polygon-compound` | Reference one immutable bounded convex or ordered convex compound and retain its local transform, center, inertia, volume, projection data, per-feature material identity, and shape identity without reconstructing geometry. | Collision supplies the immutable shape and stateless geometric operations. | Unsupported |
| `body.shape.sphere` | Retain one positive finite radius and exact sphere mass, inertia, contact, continuous-collision, drag, and fluid behavior. | Collision supplies sphere queries; callers supply physical parameters. | Unsupported |

## Body Roles

| Stable identity | Physics-owned observable contract | Required seam | Current coverage |
|---|---|---|---|
| `body.role.solid` | Admit ordinary persistent contacts and solved response under enabled pair policy. | Collision supplies geometric contacts; games supply pure pair decisions. | Unsupported |
| `body.role.trigger` | Detect ordered enter/leave transitions without ordinary impulse response and retain trigger/body identity in physical events. | Entity owns trigger lifecycle and consumes transitions. | Unsupported |
| `body.role.fluid-boundary` | Act as a trigger-shaped moving fluid boundary with one surface plane, current, physical surface, contents, and fluid controller. | PHY/Map supply records; Entity owns volume lifecycle. | Unsupported |

## Body-Pair Relationships

| Stable identity | Physics-owned observable contract | Required seam | Current coverage |
|---|---|---|---|
| `relationship.pair-exclusion` | Enable or disable one unordered stable body pair, recheck eligibility, and retire invalid persistent contacts deterministically. | Entity or game supplies pair-policy commands. | Unsupported |
| `relationship.indexed-collision-set` | Retain one nonzero set identity with a bounded element count and symmetric enabled/disabled element pairs used by multi-body assemblies. | Ragdoll and game integrations supply element indexes and rules. | Unsupported |

## Constraints And Springs

| Stable identity | Physics-owned observable contract | Required seam | Current coverage |
|---|---|---|---|
| `constraint.fixed` | Preserve one complete relative transform with shared activation, mass scaling, strength, and force/torque break behavior. | Entity or ragdoll assembly supplies endpoint identities and initial transform. | Unsupported |
| `constraint.ball-socket` | Co-locate two local anchors while permitting free relative rotation and applying shared force-break behavior. | Entity or game supplies endpoints and anchor. | Unsupported |
| `constraint.hinge` | Preserve one anchor and rotation axis with optional limits, friction or angular motor, activation, and shared break behavior. | Entity or game supplies endpoint, axis, limit, friction, and motor inputs. | Unsupported |
| `constraint.sliding` | Preserve a prismatic axis with optional linear limits, friction or motor, activation, and shared break behavior. | Entity or game supplies endpoint, frame, axis, limit, friction, and motor inputs. | Unsupported |
| `constraint.pulley` | Preserve two local anchors, two world pulley points, geared total length, rigid/slack mode, activation, and shared break behavior. | Entity or game supplies endpoints and pulley geometry. | Unsupported |
| `constraint.length` | Preserve a maximum anchor distance and optional minimum distance, including rigid equal-length mode, activation, and shared break behavior. | Entity, rope, or game integration supplies endpoints and lengths. | Unsupported |
| `constraint.ragdoll` | Preserve two constraint frames, optional translation lock, and three axes of limits, friction, motors, and clockwise interpretation. | PHY supplies runtime-neutral records; ragdoll assembly supplies bodies and frames. | Unsupported |
| `constraint.linear-spring` | Apply constant, natural length, absolute damping, relative damping, local/world anchors, and stretch-only behavior over two bodies. | Entity or game supplies endpoints and spring parameters. | Unsupported |
| `constraint.group` | Retain a bounded connected relationship group with additional iterations, error tolerance/ticks, activation, error state, and requested penetration solve. | Ragdoll or Entity supplies group membership; Simulation consumes errors. | Unsupported |

## Controllers

| Stable identity | Physics-owned observable contract | Required seam | Current coverage |
|---|---|---|---|
| `controller.motion` | Attach ordered dynamic bodies and apply local/global acceleration or local/global force at low, medium, or high priority once per fixed tick. | Entity or game supplies one pure typed controller result per attached body. | Unsupported |
| `controller.shadow` | Drive one dynamic body toward supplied position/orientation with translation/rotation permissions, speed/damping limits, arrival time, step-up, teleport distance, and physical-control mode. | Entity, Movement, StudioModel, or game supplies targets. | Unsupported |
| `controller.player-contact` | Drive one physical player proxy from Movement targets, ground velocity, push mass/speed limits, contact state, and last physical impulse without advancing player movement. | Movement supplies target state and consumes physical contact output. | Unsupported |
| `controller.vehicle-physical-wheel` | Advance bounded wheel bodies, suspension, friction, steering, braking, engine, boost, and operating state from supplied controls. | Entity/game owns controls and vehicle behavior; Collision supplies traces. | Unsupported |
| `controller.vehicle-car-raycast` | Advance car body, axle raycasts, suspension, tire friction, steering, braking, engine, boost, and operating state from supplied controls. | Entity/game owns controls; Collision supplies ordered ray results. | Unsupported |
| `controller.vehicle-jetski-raycast` | Advance the accepted jetski raycast body and contact response from supplied controls and water/ground query results. | Entity/game owns controls; Collision and Physics fluids supply query/state inputs. | Unsupported |
| `controller.vehicle-airboat-raycast` | Advance the accepted airboat raycast body, pontoon contacts, ground/water drag, steering, engine, and operating state from supplied controls. | Entity/game owns controls; Collision and Physics fluids supply query/state inputs. | Unsupported |

## Articulated And Replacement Assemblies

| Stable identity | Physics-owned observable contract | Required seam | Current coverage |
|---|---|---|---|
| `assembly.bone-follower` | Create, update, identify, and atomically destroy one ordered kinematic body per supplied solid/bone association. | PHY and StudioModel supply associations; animation supplies bone transforms; Entity consumes events. | Unsupported |
| `assembly.ragdoll` | Create, activate, advance, snapshot, sleep, and destroy at most 24 ordered bodies, joint graph, collision set, and animated-friction state. | PHY and StudioModel supply body/joint records; game owns creation and retention; Rendering consumes transforms. | Unsupported |
| `assembly.breakable-fragment-set` | Atomically replace one body with a supplied ordered set of physical fragment bodies, inherited state, and impulses while retiring prior contacts and relationships. | Entity/game owns break decision and fragment selection; presentation owners consume effects. | Unsupported |

## Generation Contract

The future checked-in generator must consume a manifest pinning every SDK declaration named by the owning roadmap, the current root Owner Registry, accepted producer inventory identities, and exact indexed target-game entity/model occurrences. It emits these 30 identities in section order, records occurrence counts and provenance, retains every `Unsupported`, `Unknown`, Malformed, and `Missing` discovery, and reproduces this file byte-for-byte on repeated runs. Generation fails on an omitted or duplicate identity, missing authority path, unassigned producer or consumer, unclassified body/relationship/controller creation call, changed fixed family count, or item-count mismatch.
