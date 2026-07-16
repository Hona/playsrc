# API Service

## Objective

Expose shared playsrc product data through a bounded network interface.

## Responsibilities

- Own API transport, authentication, request validation, and response assembly.
- Expose product-neutral catalogs and metadata required by multiple applications.

## Non-Responsibilities

- Becoming a catch-all home for domain behavior.
- Owning gameplay, asset-store, Tempus-specific, or infrastructure semantics.

## Completion

Complete when every declared endpoint is owned, bounded, observable, and backed by the module responsible for its data.
