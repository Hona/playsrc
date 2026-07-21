# Cloudflare Production PoC

## Contract

Terraform `1.15.8` and Cloudflare provider `5.22.0` own the production R2 Standard bucket, `assets.playsrc.online` R2 Custom Domain, exact-origin CORS, immutable-object Cache Rule, and Smart Tiered Cache. Wrangler `4.112.0` owns the `playsrc-web` Workers Static Assets deployment and the `playsrc.online` apex Custom Domain.

The R2 bucket contains only `objects/sha256/<lowercase-64-hex>` keys. `r2.dev` remains disabled. Successful immutable responses cache at the edge and browser for 31,536,000 seconds; non-success responses do not enter the configured edge cache. The initial checkpoint publishes the current complete BSP, WASM, PSDB, PUIB, and dependency ledger without changing their formats.

## State

Terraform state and its native lock file reside in the private `playsrc-terraform-state` R2 bucket. `bun run infra:bootstrap` creates that bucket only when Cloudflare reports it absent. The backend uses the R2 S3 endpoint supplied from `CLOUDFLARE_ACCOUNT_ID` and requires bucket-scoped S3 credentials through `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY`.

Infrastructure operations require `CLOUDFLARE_ACCOUNT_ID`, `CLOUDFLARE_ZONE_ID`, `CLOUDFLARE_API_TOKEN`, `AWS_ACCESS_KEY_ID`, and `AWS_SECRET_ACCESS_KEY`. GitHub stores the two public identifiers as repository variables. It stores the API token and S3 values as `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_R2_ACCESS_KEY_ID`, and `CLOUDFLARE_R2_SECRET_ACCESS_KEY` secrets and maps them only for deployment.

## Operations

- `bun run infra:bootstrap` creates the Terraform state bucket without changing application infrastructure.
- `bun run infra:plan` initializes the locked backend and writes one bounded production plan.
- `bun run infra:apply` replans and applies that exact plan before deleting the local plan file.
- `bun run infra:publish jump_beef` prepares, uploads, and verifies the current immutable release objects.
- `bun run infra:verify jump_beef` validates Terraform, builds the static site, and executes a Wrangler dry run without provider mutation.
- `bun run infra:deploy jump_beef` applies infrastructure, verifies every release object through `assets.playsrc.online`, deploys Workers Static Assets, and requires the public index, TF2 route, and browser configuration to agree within 600 seconds.

The GitHub `Release` workflow accepts only the next `0.0.<patch>` version, checks out the latest `main`, runs the same bootstrap and deploy commands, creates the corresponding `v0.0.<patch>` tag after live verification, and creates one GitHub Release containing the deployment manifest. Pushes to `main` never deploy.
