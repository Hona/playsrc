# Movement Modes And Parameters Inventory

Owning roadmap: [`../ROADMAP.md`](../ROADMAP.md)

Authority identity: Valve Source SDK 2013 `src/public/const.h`; `src/game/shared/igamemovement.h`, `gamemovement.{h,cpp}`, `movevars_shared.{h,cpp}`, `shareddefs.h`, `usercmd.h`, and `imovehelper.h`; `src/game/server/player_command.cpp` and `physics_main.cpp`; `src/game/client/prediction.cpp`; and TF2 ownership seams in `src/game/shared/tf/tf_gamemovement.cpp` and `tf_gamerules.cpp`, `src/game/server/tf/tf_playermove.cpp` and `tf_pushentity.cpp`, and `src/game/client/tf/tf_prediction.cpp`.

Authority revision: `88fa198fba3fb85d46d4c95018254693fdc3af0a`

Generator command: Missing

Output path: `packages/runtime/movement/inventories/modes-and-parameters.md`

Candidate item count: 101

Accepted item count: 0

Generation state: manually derived candidate; hand edits are invalid after a generator exists.

Every item below is owned by Movement as a generic behavior, validation rule, or parameter-consumption contract. “Selected game supplies” assigns the value or pure policy result, not the generic behavior. The required coverage is the disposition an implementation must establish; it does not change delivery status.

## Generic Movement Modes

Candidate count: 19.

| Stable identity | Accepted input | Required generic behavior | Supplied-value owner | Required coverage |
|---|---|---|---|---|
| `mode.none` | `MOVETYPE_NONE` | Apply the common prepass and prior-button bookkeeping, then perform no mode-specific displacement | Movement | Handled |
| `mode.walk` | `MOVETYPE_WALK` | Run ground, air, water, duck, jump, gravity, step, ladder-entry, and collision behavior | Movement | Handled |
| `mode.isometric-walk` | `MOVETYPE_ISOMETRIC` | Run the accepted generic isometric disposition; the pinned generic dispatcher selects full walk behavior | Movement; selected game decides use | Handled |
| `mode.noclip` | `MOVETYPE_NOCLIP` | Advance three-axis accelerated or direct flight without displacement collision; common water/ground categorization remains target-observable | Movement; ruleset decides permission | Handled |
| `mode.fly` | `MOVETYPE_FLY` | Run toss acceleration and swept collision without gravity | Movement | Handled |
| `mode.fly-gravity` | `MOVETYPE_FLYGRAVITY` | Run toss acceleration and swept collision with full-step gravity | Movement | Handled |
| `mode.ladder` | `MOVETYPE_LADDER` | Run attached ladder movement, jump-away, base velocity, and collision | Movement; selected game can disable | Handled |
| `mode.observer-none` | `OBS_MODE_NONE` | Preserve observer movement position and velocity and issue no movement collision query | Movement; selected game owns mode selection | Handled |
| `mode.observer-deathcam` | `OBS_MODE_DEATHCAM` | Preserve observer movement position and velocity and issue no movement collision query | Movement; selected game owns camera behavior | Handled |
| `mode.observer-freezecam` | `OBS_MODE_FREEZECAM` | Preserve observer movement position and velocity and issue no movement collision query | Movement; selected game owns camera behavior | Handled |
| `mode.observer-fixed` | `OBS_MODE_FIXED` | Preserve observer movement position and velocity and issue no movement collision query | Movement; selected game owns camera state | Handled |
| `mode.observer-in-eye` | `OBS_MODE_IN_EYE` | Copy supplied target origin, angles, and velocity from one immutable target snapshot | Movement; selected game owns target eligibility | Handled |
| `mode.observer-chase` | `OBS_MODE_CHASE` | Copy supplied target origin, angles, and velocity from one immutable target snapshot | Movement; selected game owns target eligibility | Handled |
| `mode.observer-poi` | `OBS_MODE_POI` | Copy supplied target origin, angles, and velocity from one immutable target snapshot | Movement; selected game owns target eligibility | Handled |
| `mode.observer-roaming` | `OBS_MODE_ROAMING` | Select clipped or noclip roaming from spectator policy, then apply the accepted roaming behavior | Movement; ruleset decides permission | Handled |
| `response.toss-default` | `MOVECOLLIDE_DEFAULT` | Clip toss velocity with unit overbounce and apply standable-ground stop behavior | Movement | Handled |
| `response.toss-bounce` | `MOVECOLLIDE_FLY_BOUNCE` | Clip toss velocity using the accepted surface-friction bounce response and continue eligible residual motion | Movement | Handled |
| `response.toss-custom` | `MOVECOLLIDE_FLY_CUSTOM` | Reject at the generic pure-step boundary; the pinned handler delegates velocity mutation to touch code | Selected game owns any explicit replacement | Unsupported |
| `response.toss-slide` | `MOVECOLLIDE_FLY_SLIDE` | Reject at the generic boundary; the pinned handler contains no slide case and reaches its invalid-response path | Movement | Unsupported |

`MOVETYPE_STEP`, `MOVETYPE_VPHYSICS`, `MOVETYPE_PUSH`, and `MOVETYPE_CUSTOM` are excluded from the candidate count. NPC step locomotion belongs to its game/entity owner, VPhysics belongs to Physics, mover advancement belongs to Entity/Simulation, and custom game movement belongs to the selected game.

## Shared Movement Variables

Candidate count: 19.

| Stable identity | SDK identity and pinned default | Unit/domain | Generic consumption | Required coverage |
|---|---|---|---|---|
| `cvar.sv-gravity` | `sv_gravity = 800` outside HL2 | inches/s², finite non-negative | Effective world gravity before player/game scale | Handled |
| `cvar.sv-stopspeed` | `sv_stopspeed = 100` | inches/s, finite non-negative | Minimum ground-friction control speed | Handled |
| `cvar.sv-noclipaccelerate` | `sv_noclipaccelerate = 5` | 1/s, finite | Noclip acceleration; zero and negative values retain declared direct/zero-output behavior | Handled |
| `cvar.sv-noclipspeed` | `sv_noclipspeed = 5` | multiplier, finite non-negative | Noclip command and maximum-speed factor | Handled |
| `cvar.sv-maxspeed` | `sv_maxspeed = 320` | inches/s, finite non-negative | Upper bound combined with positive selected player maximum | Handled |
| `cvar.sv-accelerate` | `sv_accelerate = 10` | 1/s, finite non-negative | Ground acceleration and pinned generic water acceleration | Handled |
| `cvar.sv-airaccelerate` | `sv_airaccelerate = 10` | 1/s, finite non-negative | Air acceleration | Handled |
| `cvar.sv-wateraccelerate` | `sv_wateraccelerate = 10` | 1/s, finite non-negative | Declared shared variable; the pinned generic movement step does not read it | Intentionally inert |
| `cvar.sv-waterfriction` | `sv_waterfriction = 1` | multiplier, finite non-negative | Declared shared variable; the pinned generic movement step does not read it | Intentionally inert |
| `cvar.sv-rollspeed` | `sv_rollspeed = 200` | inches/s, finite positive | Lateral speed producing maximum movement view roll | Handled |
| `cvar.sv-rollangle` | `sv_rollangle = 0` | degrees, finite | Maximum signed movement view roll | Handled |
| `cvar.sv-friction` | `sv_friction = 4` | 1/s, finite non-negative | Ground, pinned generic water, noclip, and clipped-observer friction | Handled |
| `cvar.sv-bounce` | `sv_bounce = 0` | multiplier, finite non-negative | First airborne wall impact and toss bounce overbounce input | Handled |
| `cvar.sv-maxvelocity` | `sv_maxvelocity = 3500` | inches/s per axis, finite positive | Symmetric per-axis velocity clamp and clipped-observer maximum | Handled |
| `cvar.sv-stepsize` | `sv_stepsize = 18` | inches, finite non-negative | Player step height copied into movement state | Handled |
| `cvar.sv-specaccelerate` | `sv_specaccelerate = 5` | 1/s, finite | Roaming-observer acceleration | Handled |
| `cvar.sv-specspeed` | `sv_specspeed = 3` | multiplier, finite non-negative | Roaming-observer command factor | Handled |
| `cvar.sv-specnoclip` | `sv_specnoclip = 1` | boolean | Select clipped or noclip roaming-observer movement | Handled |
| `cvar.sv-optimizedmovement` | `sv_optimizedmovement = 1` | boolean | Select the target initial position-categorization fast path; query and state effects remain observable | Handled |

`sv_footsteps`, `sv_backspeed`, and `sv_waterdist` are excluded from the candidate count: actual footstep effects belong to game/audio consumers, backward-speed policy is game-specific, and near-water view correction belongs to presentation. Sky and vehicle variables declared beside movement variables are also excluded.

## Explicit Policy Inputs

Candidate count: 23.

| Stable identity | Required value | Generic consumption | Supplied-value owner | Required coverage |
|---|---|---|---|---|
| `policy.tick-interval` | Positive finite seconds | Every acceleration, friction, gravity, displacement, and timer transition in one step | Simulation | Handled |
| `policy.command-number` | Non-negative safe integer plus stable player identity | Deterministic interval selection and prediction comparison identity | Simulation/Networking | Handled |
| `policy.hull-standing` | Finite mins/maxs with `mins <= maxs` on every axis | Standing sweeps, position tests, ground quadrants, and water samples | Selected game | Handled |
| `policy.hull-crouched` | Finite mins/maxs with `mins <= maxs` on every axis | Crouched sweeps, position tests, duck origin shifts, and water samples | Selected game | Handled |
| `policy.hull-observer` | Finite mins/maxs with `mins <= maxs` on every axis | Clipped observer sweeps and position tests | Selected game | Handled |
| `policy.view-standing` | Finite Source-space offset | Standing view output and eye-water sample | Selected game | Handled |
| `policy.view-crouched` | Finite Source-space offset | Duck interpolation and crouched eye-water sample | Selected game | Handled |
| `policy.view-dead` | Finite Source-space offset | Dead-state generic view output | Selected game | Handled |
| `policy.model-scale` | Finite positive scalar | Applied once to hulls and view offsets | Selected game | Handled |
| `policy.player-gravity-scale` | Finite non-negative scalar; zero selects the accepted default semantic | Effective gravity for this player | Selected game/entity state | Handled |
| `policy.lagged-movement-scale` | Finite non-negative scalar | Multiplies the fixed tick interval for this movement transition only | Selected game | Handled |
| `policy.client-max-speed` | Finite non-negative inches/s; zero means no client cap | Combined before surface/constraint speed factors | Networking/game state | Handled |
| `policy.game-max-speed` | Finite non-negative inches/s | Selected player maximum and game post-clamp policy | Selected game | Handled |
| `policy.surface-friction` | Finite non-negative scalar | Ground/air/water friction and acceleration plus bounce | Material supplies; Movement remaps | Handled |
| `policy.surface-max-speed-factor` | Finite non-negative scalar | Multiplies effective maximum speed before command clamp | Material | Handled |
| `policy.surface-jump-factor` | Finite non-negative scalar | Multiplies generic jump impulse | Material | Handled |
| `policy.surface-climbable` | Boolean | Ladder eligibility when ladder contents are absent | Material | Handled |
| `policy.air-speed-cap` | Finite non-negative inches/s; generic value 30 | Caps projected add speed while retaining uncapped wish speed for acceleration | Movement default; selected game can override | Handled |
| `policy.duck-duration` | Finite non-negative seconds | Grounded duck transition and reversal timeline | Movement default; selected game can override | Handled |
| `policy.unduck-duration` | Finite non-negative seconds | Grounded unduck transition, reversal, and eye timeline | Movement default; selected game can override | Handled |
| `policy.movement-constraint` | Finite center, non-negative radius/width, finite non-negative outward factor | Slows only outward wish motion inside the declared annulus | Entity/game state supplies; Movement consumes | Handled |
| `policy.player-collision` | Unsigned mask, collision group, stable ignored identity, and pure game predicate | Every player hull/ground/ladder/stuck query | Selected game supplies; Collision executes | Handled |
| `policy.movement-optimizations` | Pinned target value `true` | Select optimized interval, command-clamp, basis, jump-impulse, trace-reuse, and point-contents-cache paths while preserving their exact arithmetic | Movement | Handled |

## Limits, Thresholds, Factors, Distances, And Timers

Candidate count: 40.

| Stable identity | Pinned generic value | Unit/domain | Required behavior | Required coverage |
|---|---:|---|---|---|
| `limit.move-bumps` | 4 | traces per slide operation | Stop after four attempted remaining-time sweeps | Handled |
| `limit.clip-planes` | 5 | planes per slide operation | Stop and zero velocity before adding a sixth plane | Handled |
| `threshold.standable-normal-z` | 0.7 | unit-normal Z | Values at or above are standable where the caller uses inclusive comparison; each strict comparison retains its declared boundary | Handled |
| `distance.ground-probe` | 2 | inches | Categorization probes two inches below origin | Handled |
| `distance.step-epsilon` | 0.03125 | inches | Add to upward and downward step sweeps | Handled |
| `threshold.ground-network-snap` | 0.015625 | inches | Do not publish sub-threshold stay-on-ground Z changes | Handled |
| `threshold.motion-stop` | 0.1 | inches/s | Generic friction and water paths retain their exact strict boundary | Handled |
| `threshold.walk-stop` | 1 | inches/s | Unobstructed walk total speed below the boundary stops movement | Handled |
| `threshold.leave-ground-speed` | 140 | inches/s upward relative to support | Rapid upward motion clears ground; selected game can replace | Handled |
| `threshold.optimized-ground-clear-speed` | 250 | inches/s upward | The optimized walk pre-pass clears ground before mode dispatch above this strict boundary | Handled |
| `jump.default-height` | 21 | inches | Generic jump impulse derives from effective gravity and this height when the generic policy is selected | Handled |
| `timer.duck-master` | 1000 | milliseconds | Stores reversible duck/unduck transition progress | Handled |
| `timer.jump-duck` | 510 | milliseconds | Generic duck-jump transition timer | Handled |
| `factor.duck-command` | 0.33333333 | scalar | Crop grounded crouched forward/side/up command once | Handled |
| `distance.water-feet-offset` | 1 | inch above hull minimum | First water-level point sample | Handled |
| `factor.water-current` | 50 × water level | inches/s | Add each selected directional current to base velocity once | Handled |
| `speed.water-idle-sink` | 60 | inches/s downward wish | Apply only with no forward, side, or up command and no jump-up intent | Handled |
| `factor.water-wish-speed` | 0.8 | scalar | Scale water wish speed after maximum-speed clamp | Handled |
| `speed.water-swim-jump` | 100 | inches/s upward | Generic jump-held velocity in water contents | Handled |
| `speed.slime-swim-jump` | 80 | inches/s upward | Generic jump-held velocity in slime contents | Handled |
| `distance.water-ledge-forward` | 24 | inches | Waist and eye ledge-exit sweeps | Handled |
| `distance.water-ledge-height` | 8 | inches | Eye-clearance offset above view position | Handled |
| `speed.water-ledge-up` | 256 | inches/s | Generic ledge-exit initial upward velocity | Handled |
| `timer.water-ledge-duration` | 2000 | milliseconds | Generic ledge-exit state duration | Handled |
| `distance.ladder-attach` | 2 | inches | Maximum generic hull sweep distance for ladder attachment | Handled |
| `speed.ladder-climb` | 200 | inches/s | Generic forward/right button climb speed | Handled |
| `factor.ladder-lateral` | 1 | scalar | Generic lateral ladder contribution | Handled |
| `speed.ladder-jump-away` | 270 | inches/s | Jump detaches along ladder plane normal | Handled |
| `interval.ground-surface` | 0.3 | seconds converted to command ticks | Target-compatible periodic ground-surface refresh | Handled |
| `interval.stuck-multiplayer` | 1 | seconds converted to command ticks | Multiplayer full stuck-check cadence when not recovering | Handled |
| `interval.stuck-singleplayer` | 0.2 | seconds converted to command ticks | Single-player full stuck-check cadence when not recovering | Handled |
| `interval.stuck-min-time` | 0.05 | seconds | Target wall-clock retry throttle; blocked pending deterministic mapping | Handled |
| `stuck.offset-sequence` | 54 ordered offsets | Source-space inches | Test the retained little/big offset sequence cyclically and reset on recovery | Handled |
| `interval.ladder-check` | 0.2 | seconds converted to command ticks | Target-compatible periodic ladder check when optimization is selected | Handled |
| `factor.noclip-speed-button` | 0.5 | scalar | Halve noclip factor while the speed button is held | Handled |
| `factor.observer-speed-button` | 0.5 | scalar | Halve roaming-observer factor while the speed button is held | Handled |
| `distance.stay-ground-rise` | 2 | inches | Probe upward before the downward stay-on-ground sweep | Handled |
| `distance.water-landing-probe` | 1024 | inches downward | Require a standable landing below ledge-exit eye clearance | Handled |
| `timer.water-ledge-maximum` | 10000 | milliseconds | Clamp malformed/excess retained water-jump timer before reduction | Handled |
| `factor.surface-friction-remap` | multiply by 1.25, clamp to 1 | scalar | Convert supplied physics surface friction to generic player surface friction | Handled |

`stuck.offset-sequence` is the exact 54-slot sequence below. Each range is inclusive and advances in the listed order; nested axes are outermost to innermost as written.

1. Slots 0–2: `(0, 0, z)` for `z = -0.125, 0, 0.125`.
2. Slots 3–5: `(0, y, 0)` for `y = -0.125, 0, 0.125`.
3. Slots 6–8: `(x, 0, 0)` for `x = -0.125, 0, 0.125`.
4. Slots 9–16: `(x, y, z)` for `x`, then `y`, then `z` in `-0.125, 0.125`.
5. Slots 17–19: `(0, 0, z)` for `z = 0, 1, 6`.
6. Slots 20–22: `(0, y, 0)` for `y = -2, 0, 2`.
7. Slots 23–25: `(x, 0, 0)` for `x = -2, 0, 2`.
8. Slots 26–52: `(x, y, z)` for `z = 0, 1, 6`, then `x = -2, 0, 2`, then `y = -2, 0, 2`.
9. Slot 53: `(0, 0, 0)`.

## Generation Contract

The future generator must:

1. Parse the named movement-mode, observer-mode, move-variable, move-data, generic constant, limit, and policy declarations from a retained SDK authority snapshot at the recorded revision.
2. Compare generic declarations with TF2 overrides only to assign the value/policy owner. It must not emit TF2 behavior as Movement-owned.
3. Emit the four sections in this file and stable identities in the shown order with exact values, units, dispositions, supplied-value owners, and exclusions.
4. Fail on a changed value, duplicate identity, unclassified movement or observer mode, newly consumed shared variable, removed intentionally inert variable, undeclared game override, owner conflict, malformed numeric expression, or missing authority input.
5. Record authority identity, revision, generator command, output path, owning roadmap, exact item count, and every `Handled`, `Intentionally inert`, `Unsupported`, `Malformed`, `Unknown`, or `Missing` classification.
