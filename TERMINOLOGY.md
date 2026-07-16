# Terminology

This glossary defines language used across playsrc code, documentation, issues, reviews, and roadmaps.

## Engine And Game

**Source 1**: The engine family targeted by playsrc.

**Source 2**: Valve's later engine family. Explicitly out of scope.

**Game**: A Source 1 title with its own content, entities, movement differences, weapons, rules, and networking behavior.

**Game module**: The module implementing one game's behavior over generic Source packages.

**Ruleset**: Mode-specific rules owned by one game, such as TF2 jump or CS:S surf. Identically named rulesets in different games are separate.

**Content build**: A factual installed game-content state used as input.

## Authority

**Gameplay**: Authoritative movement, entities, weapons, damage, physics, and game rules, whether local or networked.

**Multiplayer**: Multiple players participating in one gameplay simulation.

**Online multiplayer**: Networked clients connected to a hosted gameplay server.

**Replay**: Presentation of parsed demo state without resimulating gameplay.

**Gameplay authority**: The single implementation that advances gameplay state.

**Replay authority**: Parsed demo state used during replay presentation.

**Rendering**: Presentation of gameplay or replay state. Rendering never invents authoritative state.

**Presentation state**: Interpolated or derived visual state that cannot affect gameplay.

**Simulation tick**: One fixed authoritative gameplay step.

**Render frame**: One browser or GPU presentation update.

## Parity

**Parity**: Matching observable Source or game behavior for the same input and state.

**Behavioral parity**: Matching gameplay, entity, physics, timing, or state-transition behavior.

**Visual parity**: Matching rendered output under aligned map state, camera, tick, viewport, and render settings.

**Parity claim**: A statement that defined behavior matches its target and has credible evidence.

**Approximation**: Behavior intentionally not exact. Approximations are not parity.

**Complete**: The declared behavior family is implemented through compilation, runtime consumption, and appropriate evidence.

**Partial**: Some declared behavior exists while required cases remain absent.

## Coverage

**Handled**: playsrc implements the encountered behavior.

**Intentionally inert**: The value is valid and correctly requires no behavior in the current context.

**Unsupported**: The value is understood but its required behavior is not implemented.

**Malformed**: The input violates its expected format or contract.

**Unknown**: The value is not recognized or classified.

**Missing**: A required input or artifact is absent.

**Ready**: The item passed its required technical validation and can be consumed.

These are technical states, not legal or publication classifications.

## Data And Formats

**Source data**: Raw game or map data consumed by playsrc.

**Content**: Mounted raw Source resources resolved by exact logical path and declared precedence.

**Compiled data**: Data emitted by Source compilation tools, including BSP faces, planes, nodes, visibility, lightmaps, and entities.

**Reconstruction**: Data inferred from another representation when exact compiled data is unavailable.

**Logical path**: A Source-style resource identity such as `materials/example/material.vmt`.

**Provenance**: Factual information identifying where bytes came from.

**Content hash**: A digest identifying exact bytes.

**Parser**: Code that reads a Source binary or text format without applying product behavior.

**Compiler**: Code that converts parsed Source data into canonical or runtime-ready output.

**Canonical representation**: A playsrc-owned semantic representation of a Source domain.

**IR**: Intermediate representation between parsing and runtime-specific output.

**Adapter**: Code translating one domain interface into another environment without redefining the underlying behavior.

## Packages And Assets

**Package**: An independently useful playsrc module with a defined interface.

**Map package**: Immutable compiled map data and references required to load one map.

**Artifact**: A generated file or object produced by compilation.

**Object**: Immutable bytes stored by content hash.

**CAS**: Content-addressed storage in which objects are stored and retrieved by content hash.

**Asset graph**: Relationships between logical Source resources, compiled artifacts, and content-addressed objects.

**Root manifest**: An immutable document identifying the complete object graph for a map, content build, or application.

**Catalog**: Application metadata selecting maps or experiences without defining map truth.

**Channel**: A mutable pointer selecting an immutable root for development, staging, or production.

**Source cache**: Reusable raw conversion inputs. It is disposable and never deployed.

**Work directory**: Ephemeral files owned by one active job.

**Asset store**: Durable content-addressed compiled output used by applications and deployment.

**CDN**: A remote published mirror of the asset store.

**Browser cache**: Client-side HTTP caching, separate from the Source cache and asset store.

## Architecture

**Domain**: A cohesive Source responsibility such as BSP, materials, models, entities, physics, or rendering.

**Module**: An implementation unit owned by one domain.

**Mental model**: The smallest coherent set of concepts a maintainer must understand to work within one module.

**Interface**: Everything callers must know to use a module correctly, including inputs, outputs, invariants, ordering, errors, limits, and performance characteristics.

**Primitive**: A reusable capability with no product assumptions.

**Application**: A deployed web or network program assembled from playsrc packages, a game, and any selected game-owned ruleset.

**Product**: A user-facing experience delivered by one or more applications.

**Tool**: A program run by developers or operators rather than continuously deployed for users.

**Infrastructure**: Checked definitions for hosting resources and environments used by applications.

**Fallback**: A secondary implementation used when the intended implementation fails. Targeted playsrc behavior does not use fallbacks.

**Legacy path**: A replaced implementation retained alongside its replacement. Legacy paths should be deleted.

**Compatibility layer**: Code maintaining an older contract for existing consumers. playsrc currently does not maintain compatibility layers.

## Verification

**Fixture**: Stable input data used during verification.

**Vector**: An input and expected observable output pair.

**Synthetic vector**: A project-created minimal vector isolating one behavior.

**Capture**: Recorded observable output from a controlled run.

**Evidence**: Tests, vectors, captures, measurements, or fair manual inspection supporting a claim.

**Deterministic evidence**: Evidence reproducible from fixed inputs without uncontrolled variation.

**Manual inspection**: Human evaluation used when automation would be unfair or misleading.

**Regression**: Previously working or evidenced behavior that no longer works.

## Performance And Workflow

**Bound**: An explicit maximum for memory, time, concurrency, queue depth, or output size.

**Backpressure**: Mechanisms preventing producers from overwhelming consumers or resources.

**Cold run**: Execution without reusable cache state.

**Warm run**: Execution with valid reusable cache state.

**Frame pacing**: Distribution and consistency of render-frame timing.

**Tick time**: Time required to execute one gameplay simulation tick.

**Current contract**: The only contract supported during private development.

**Breaking change**: A change requiring every producer and consumer to update together.

**Stale artifact**: Generated output that no longer matches current code or inputs and should be regenerated.

**Checkpoint**: A coherent commit representing completed work.

**Roadmap item**: A declared parity requirement tracked by its owning module.

**Exit criteria**: Conditions required before a roadmap item or module is marked complete.
