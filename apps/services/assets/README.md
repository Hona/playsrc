# Asset Service

## Objective

Serve immutable raw Source objects, reproducible derived objects, descriptors, catalogs, and channels locally and remotely.

## Responsibilities

- Translate HTTP requests into bounded asset-store reads.
- Provide byte-range access required by unchanged BSP and VPK objects plus immutable caching semantics, metadata, and operational observability.
- Serve local development and deployed asset origins through one application contract.

## Non-Responsibilities

- Defining CAS identity, compiling artifacts, or choosing application catalog contents.
- Provisioning CDN or storage infrastructure.

## Completion

Complete when declared asset-store resources are served correctly with bounded requests and cache behavior.
