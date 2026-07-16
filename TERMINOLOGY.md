# Terminology

This file is normative for playsrc code, documentation, issues, reviews, and roadmaps. A defined term must not be used with a different meaning. An undefined or disputed concept must be named as an unknown or an explicit decision instead of being hidden behind a broader term.

## Engine And Game

**Source 1**: The Valve engine generation used by the declared playsrc game targets: Team Fortress 2, Counter-Strike: Source, and legacy Source 1 Counter-Strike: Global Offensive.

**Source 2**: Valve's successor engine generation. Source 2 formats, behavior, interfaces, terminology, compatibility, and future-proofing are excluded from playsrc.

**Clean-room port**: A public implementation containing independently authored code plus code copied or adapted only from sources whose licenses permit inclusion. Restricted implementation text, comments, implementation-specific symbols, file structure, and control flow never enter the public implementation.

**Game**: One Source 1 title with a distinct content build, player state, movement differences, entities, items, weapons, objectives, rules, prediction state, and networked state.

**Game module**: The sole owner of behavior that differs by game. A game module composes generic Source packages and owns the game's classes, items, attributes, weapons, projectiles, entities, objectives, game rules, replicated state, prediction behavior, and presentation mappings.

**Ruleset**: A module owned by one game that defines mode lifecycle, objectives, scoring, spawning, restrictions, completion, and game-state transitions. Rulesets with the same name under different games never share a ruleset implementation. Reusable game-independent mechanics belong in packages.

**Content build**: One upstream game-content release identified by game and the build identity exposed by its source. If the source exposes no build identity, that field is Unknown. Every consumed file retains its own provenance and SHA-256 hash. A content build identifies data provenance; it is not an interface version and does not require scanning an installation.

## Authority

**Gameplay**: Deterministic advancement of player, entity, weapon, projectile, damage, movement, physics, objective, and game-rule state from a prior state, simulation tick, commands, and immutable game content.

**Multiplayer**: Two or more players participating in the same gameplay authority.

**Online multiplayer**: Multiplayer in which at least one player command or authoritative snapshot crosses a network transport between a client and a hosted gameplay server.

**Replay**: Advancement of presentation state by decoding recorded demo data. Replay never advances gameplay from player commands and never substitutes gameplay resimulation for recorded state.

**Gameplay authority**: The single simulation implementation permitted to advance gameplay state for one session. Rendering, replay, networking transports, applications, and inspectors cannot mutate that state independently.

**Replay authority**: The single decoded recorded-state timeline permitted to determine replay state for one playback session.

**Rendering**: Production of pixels and GPU work from canonical world data plus gameplay, replay, or presentation state. Rendering cannot create gameplay or replay truth.

**Presentation state**: Interpolated, animated, culled, or otherwise derived state consumed only by rendering, particles, or audio. Presentation state cannot affect gameplay authority or replay authority.

**Simulation tick**: One fixed-duration gameplay-authority transition from prior state and commands to next state and events.

**Render frame**: One presentation update submitted for display. A render frame can interpolate between simulation ticks and cannot advance gameplay.

## Parity

**Parity**: A passing declared comparison predicate between target output and playsrc output for the same content build, initial state, inputs, and configuration. A parity statement without those declarations is not a parity claim.

**Behavioral parity**: Parity of state transitions, events, collision results, movement results, damage results, entity I/O, objective state, timing, or network-visible state.

**Visual parity**: Parity of rendered output under the same map state, presentation state, camera transform, simulation tick, viewport, render configuration, asset set, and comparison method.

**Parity claim**: A statement naming the target behavior, target content build, covered inputs, playsrc output, comparison method, evidence location, and numeric tolerance whenever the comparison predicate is not exact byte or value equality.

**Approximation**: An implementation known to produce output different from the parity target. An approximation cannot satisfy a parity roadmap item.

**Completion denominator**: The finite set of required roadmap items whose status determines completion for one declared target.

**Complete**: Every item in the target's completion denominator is Ready, every required producer and consumer uses the current implementation, every exit criterion passes, and no required item remains Unsupported, Unknown, Missing, Partial, or Blocked.

**Partial**: At least one required behavior is implemented and at least one required behavior in the same declared item remains unimplemented or unintegrated.

## Coverage Classifications

Every encountered value, record, entity class, and behavior receives exactly one classification: Handled, Intentionally inert, Unsupported, Malformed, Unknown, or Missing.

**Handled**: The owning module produces every required output and state transition for the classified input in the declared context, and every required consumer uses that result.

**Intentionally inert**: The input is valid and classified, and the target behavior requires no state transition, emitted event, generated artifact, or presentation effect in the declared context.

**Unsupported**: The input and required target behavior are identified, but playsrc does not implement every required output or state transition.

**Malformed**: The input violates a structural invariant of its declared format or interface. Malformed input is neither Unknown nor Unsupported.

**Unknown**: The parser accepts the input's declared structural syntax, but the owning module cannot assign a defined semantic classification.

**Missing**: A required input or artifact was not supplied and could not be resolved at its exact configured identity.

These classifications describe technical support only. They never classify legal status, ownership, or publication permission.

## Delivery Status

Every roadmap item has exactly one delivery status: Not started, In progress, Blocked, or Ready.

**Not started**: No implementation, integration, or evidence work has begun for the roadmap item's current requirement. Research and roadmap definition do not change this status.

**In progress**: Implementation, integration, or evidence work has begun, at least one Ready predicate remains false, and no recorded blocker prevents the next required action.

**Blocked**: The next required action cannot proceed because a named behavior, input, dependency, decision, or evidence method is unavailable. The roadmap item records the exact missing requirement and every location or source already checked.

**Ready**: The item satisfies its owning roadmap row, required integrations, declared validation, and evidence requirements and is consumable through the current interface.

## Data And Formats

**Source data**: Exact bytes read from a configured Source content provider before playsrc parsing or compilation.

**Content**: The ordered set of explicitly configured providers used to resolve Source logical paths. Content contains raw Source bytes and never contains playsrc-compiled output.

**Source-compiled data**: Output emitted by Source compilation tools. BSP geometry, planes, nodes, leaves, visibility, lightmaps, and entity lumps are Source-compiled data.

**playsrc-compiled data**: Deterministic output emitted by a playsrc compiler from declared Source bytes, compiler behavior, and build configuration.

**Reconstruction**: A value inferred because the authoritative Source representation is absent. Reconstructed data is labeled as reconstructed and never presented as exact compiled data.

**Logical path**: A slash-separated Source resource identity resolved inside configured content providers, for example `materials/example/material.vmt`. A logical path is not an operating-system path and never authorizes filesystem discovery.

**Provenance**: The game, content build, logical path, provider identity, exact source location inside that provider, and SHA-256 hash identifying where consumed bytes came from.

**Content hash**: The lowercase hexadecimal SHA-256 digest of exact object bytes.

**Parser**: A module that validates and decodes one declared binary or text format into typed format data. A parser does not apply game, ruleset, application, rendering, or deployment behavior.

**Compiler**: A deterministic transformation from declared parsed or semantic inputs, compiler behavior, and build configuration to playsrc-compiled data.

**Canonical representation**: The sole playsrc-owned semantic representation of one domain consumed by downstream modules. Derived transport and presentation formats cannot override it.

**Adapter**: An implementation at a seam that translates an existing interface to one concrete environment or provider. An adapter cannot redefine the behavior owned behind the interface.

## Packages And Assets

**Package**: An independently consumable module under `packages/` with one documented interface, one owning mental model, and no game or application assumptions.

**Map package**: An immutable map root manifest plus references to every object required to load the declared map representation. A map package never embeds duplicate copies of shared objects.

**Artifact**: Any file, byte sequence, manifest, report, or generated source emitted by a playsrc operation.

**Object**: One immutable byte sequence addressed by its SHA-256 content hash in the asset store.

**CAS**: Storage implementing the invariant that one content hash maps to one exact immutable byte sequence and that sequence is physically stored once per store.

**Asset graph**: The directed graph whose nodes are immutable objects and roots and whose edges are hash references. Reachability from retained roots determines which objects must be preserved.

**Root manifest**: An immutable object naming the complete reachable object graph for one map, game content build, or application build.

**Catalog**: Application-owned metadata selecting immutable roots and presenting maps or experiences. A catalog cannot alter map, game, or artifact truth.

**Channel**: One mutable name pointing atomically to one immutable root. Development, staging, and production channels are independent names.

**Source cache**: Disposable storage for reusable raw Source inputs. Source-cache contents are never referenced by published manifests and are never deployed.

**Work directory**: One temporary directory owned by one process tree and one operation. It is deleted after success or failure and is never referenced by a manifest.

**Asset store**: Durable storage containing immutable objects, roots, catalogs, channels, validation metadata, and publication state. Deleting an object reachable from a retained root is prohibited.

**CDN**: A remote HTTP-accessible mirror of published asset-store objects, roots, catalogs, and channels.

**Browser cache**: HTTP cache state owned by a browser. It is not the Source cache, work directory, asset store, or CDN.

## Architecture

**Domain**: One named responsibility with its own vocabulary, invariants, behavior family, and owner.

**Module**: One interface and the implementation that exclusively owns the behavior behind that interface. The term applies to packages, games, rulesets, applications, tools, and infrastructure definitions.

**Mental model**: The complete set of concepts a maintainer must understand to change one module without reading unrelated implementations.

**Interface**: Every fact a caller must know to use a module correctly: accepted inputs, returned outputs, state transitions, ordering, errors, invariants, limits, configuration, ownership, and performance characteristics.

**Application**: A deployable web or network program under `apps/`. Applications assemble packages, one game when gameplay is present, and one game-owned ruleset when mode behavior is present.

**Product**: A named user experience delivered by one or more applications. TF2, CS:S, legacy CS:GO, and Tempus are products.

**Tool**: An executable under `tools/` invoked by a developer or operator. A tool calls module interfaces and cannot become an alternate implementation of package, game, ruleset, or application behavior.

**Infrastructure**: Checked definitions under `infra/` that provision or configure CDN, storage, compute, networking, routing, DNS, data, observability, and environments. Infrastructure does not own application or domain behavior.

**Fallback**: A second implementation selected after the current implementation fails. Targeted playsrc behavior has no fallback.

**Legacy path**: A replaced implementation retained beside the current implementation. A completed change contains no legacy path.

**Compatibility layer**: Code preserving an older interface, schema, artifact reader, or behavior after the current contract changes. playsrc does not maintain compatibility layers before external consumers exist.

## Verification

**Fixture**: Immutable input bytes stored with their SHA-256 hash, provenance, declared purpose, and owning verification case.

**Vector**: A fixed input, execution configuration, expected observable output, and comparison method.

**Synthetic vector**: A vector authored by playsrc to isolate one named behavior rather than reproduce a complete game-content example.

**Capture**: Recorded observable output stored with the exact content build, inputs, configuration, tool commit, operating system, CPU architecture, GPU and driver for GPU output, browser and version for browser output, runtime version, and capture procedure.

**Evidence**: A named test result, vector result, capture, measurement, or manual-inspection record that directly supports one roadmap claim.

**Deterministic evidence**: Evidence that produces the same compared output from fixed bytes, configuration, tool commit, and execution procedure.

**Manual inspection**: A recorded human comparison with fixed inputs, fixed procedure, explicit acceptance criteria, observer result, and retained output. If output retention fails, the record names the exact failed operation and error.

**Regression**: A current result that fails behavior or evidence previously satisfied by the same declared interface and comparison method.

## Performance And Workflow

**Bound**: A declared numeric maximum for memory, time, concurrency, queue depth, input size, or output size under a named operation.

**Backpressure**: A declared mechanism that stops, delays, rejects, or bounds producers when a consumer or resource reaches its capacity.

**Cold run**: An operation started with no reusable cache entry for its declared inputs.

**Warm run**: An operation started with every reusable cache entry required by its declared inputs present and valid.

**Frame pacing**: The ordered distribution of render-frame intervals measured for a named scene, camera path, viewport, render configuration, content build, device, and time window.

**Tick time**: Wall-clock duration required to execute one simulation tick for a named state, command set, build, and machine.

**Current contract**: The only interface, schema, and artifact shape accepted by the current repository state.

**Breaking change**: A change that requires at least one producer, consumer, schema, fixture, generated artifact, or documented interface to change in the same checkpoint.

**Stale artifact**: Generated output whose recorded inputs, build configuration, or tool commit differs from the current required values.

**Checkpoint**: One commit in which the selected behavior family, required documentation, producers, consumers, generated artifacts, and evidence agree on the current contract.

**Inventory**: A finite, reproducible enumeration generated from named authoritative inputs. An inventory records its source identity and generation procedure.

**Roadmap item**: One falsifiable requirement owned by one module and expressed as target behavior, playsrc behavior, evidence, and status.

**Exit criteria**: The complete set of predicates that must all be true before one roadmap item, module, game, ruleset, application, tool, infrastructure target, or product is Complete.
