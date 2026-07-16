# Infrastructure

## Objective

Define the shared hosting resources and environments on which playsrc applications run.

## Responsibilities

- Define CDN, storage, compute, networking, routing, DNS, data, observability, and environment resources.
- Bind deployed applications to provisioned resources through explicit configuration.
- Keep infrastructure changes reproducible and reviewable.

## Non-Responsibilities

- Compiling Source content or defining asset-store and channel semantics.
- Owning application behavior or release orchestration.
- Combining compilation with deployment.

## Relationships

Applications own application-specific deployment configuration; `tools/playsrc` owns release commands; packages own reusable behavior.

## Completion

Complete when every declared environment can be provisioned, observed, changed, and retired through checked definitions.
