# Particle

## Objective

Represent and advance Source particle definitions independently of game and GPU implementations.

## Responsibilities

- Parse and classify particle definitions, operators, initializers, emitters, and child systems.
- Advance particle state from explicit events and deterministic inputs where required.
- Produce runtime-neutral particle presentation data.

## Non-Responsibilities

- Selecting TF2-specific effects for gameplay events.
- Owning GPU draw resources or gameplay state.
- Hiding unsupported operators behind visual approximations.

## Relationships

Game modules bind gameplay events to effects; rendering presents particle output; content resolves particle resources.

## Completion

Complete when the declared particle definition and behavior inventories are classified, implemented, and fairly verified.
