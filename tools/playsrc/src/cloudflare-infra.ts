import { rm } from "node:fs/promises"
import path from "node:path"
import { CLOUDFLARE_ASSET_BUCKET, CloudflareError, runWrangler, WRANGLER_CONFIG } from "./cloudflare"
import { repositoryRoot } from "./config"

const TERRAFORM_VERSION = "1.15.8"
const TERRAFORM_DIRECTORY = path.join(repositoryRoot, "infra", "cloudflare")
const PLAN_PATH = path.join(TERRAFORM_DIRECTORY, "production.tfplan")
const TERRAFORM_TIMEOUT_MILLISECONDS = 30 * 60 * 1_000
const REQUIRED_ENVIRONMENT = [
  "CLOUDFLARE_ACCOUNT_ID",
  "CLOUDFLARE_ZONE_ID",
  "CLOUDFLARE_API_TOKEN",
  "AWS_ACCESS_KEY_ID",
  "AWS_SECRET_ACCESS_KEY",
] as const

type TerraformResult = Readonly<{ code: number; stdout: string; stderr: string }>

function terraformEnvironment(): NodeJS.ProcessEnv {
  for (const name of REQUIRED_ENVIRONMENT) {
    if (!process.env[name]) throw new CloudflareError(`${name} is required for Cloudflare infrastructure operations`)
  }
  return {
    ...process.env,
    TF_IN_AUTOMATION: "1",
    TF_INPUT: "0",
    TF_VAR_cloudflare_account_id: process.env.CLOUDFLARE_ACCOUNT_ID,
    TF_VAR_cloudflare_zone_id: process.env.CLOUDFLARE_ZONE_ID,
    AWS_ENDPOINT_URL_S3: `https://${process.env.CLOUDFLARE_ACCOUNT_ID}.r2.cloudflarestorage.com`,
  }
}

async function runTerraform(args: readonly string[], environment = terraformEnvironment()): Promise<TerraformResult> {
  const executable = Bun.which("terraform")
  if (!executable) throw new CloudflareError(`Terraform ${TERRAFORM_VERSION} is required on PATH`)
  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), TERRAFORM_TIMEOUT_MILLISECONDS)
  try {
    const child = Bun.spawn([executable, `-chdir=${TERRAFORM_DIRECTORY}`, ...args], {
      cwd: repositoryRoot,
      env: environment,
      stdout: "pipe",
      stderr: "pipe",
      signal: controller.signal,
    })
    const [stdout, stderr, code] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
      child.exited,
    ])
    return Object.freeze({ code, stdout, stderr })
  } catch (error) {
    if (controller.signal.aborted) throw new CloudflareError("Terraform exceeded its 30 minute operation bound")
    throw new CloudflareError(error instanceof Error ? error.message : "Terraform could not start")
  } finally {
    clearTimeout(timeout)
  }
}

function requireResult(result: TerraformResult, operation: string): void {
  if (result.stdout) console.error(result.stdout.trim())
  if (result.code !== 0) {
    const detail = result.stderr.trim()
    throw new CloudflareError(`${operation} failed${detail ? `: ${detail}` : ""}`)
  }
}

async function requireTerraformVersion(): Promise<void> {
  const result = await runTerraform(["version", "-json"])
  requireResult(result, "Terraform version check")
  let version: unknown
  try {
    version = (JSON.parse(result.stdout) as { terraform_version?: unknown }).terraform_version
  } catch {}
  if (version !== TERRAFORM_VERSION) throw new CloudflareError(`Terraform ${TERRAFORM_VERSION} is required, found ${String(version)}`)
}

async function initializeTerraform(): Promise<void> {
  await requireTerraformVersion()
  const result = await runTerraform(["init", "-input=false"])
  requireResult(result, "Terraform initialization")
}

export async function bootstrapCloudflareState(): Promise<void> {
  const name = "playsrc-terraform-state"
  const existing = await runWrangler(["r2", "bucket", "info", name, "--json", `--config=${WRANGLER_CONFIG}`])
  if (existing.code === 0) return
  const output = `${existing.stderr}\n${existing.stdout}`
  if (!/(?:not found|does not exist|10006)/iu.test(output)) {
    throw new CloudflareError(`Terraform state bucket check failed: ${output.trim()}`)
  }
  const created = await runWrangler(["r2", "bucket", "create", name, `--config=${WRANGLER_CONFIG}`])
  if (created.code !== 0) throw new CloudflareError(`Terraform state bucket creation failed: ${created.stderr.trim()}`)
}

export async function planCloudflareInfrastructure(): Promise<void> {
  await initializeTerraform()
  await rm(PLAN_PATH, { force: true })
  const result = await runTerraform(["plan", "-input=false", `-out=${PLAN_PATH}`])
  requireResult(result, "Terraform plan")
}

export async function applyCloudflareInfrastructure(): Promise<void> {
  await planCloudflareInfrastructure()
  const result = await runTerraform(["apply", "-input=false", "-auto-approve", PLAN_PATH])
  try {
    requireResult(result, "Terraform apply")
  } finally {
    await rm(PLAN_PATH, { force: true })
  }
  const bucket = await runWrangler(["r2", "bucket", "info", CLOUDFLARE_ASSET_BUCKET, "--json", `--config=${WRANGLER_CONFIG}`])
  if (bucket.code !== 0) throw new CloudflareError("Terraform apply did not expose the production asset bucket")
}

export async function validateCloudflareInfrastructure(): Promise<void> {
  const environment = { ...process.env, TF_IN_AUTOMATION: "1", TF_INPUT: "0" }
  const executable = Bun.which("terraform")
  if (!executable) throw new CloudflareError(`Terraform ${TERRAFORM_VERSION} is required on PATH`)
  const version = await runTerraform(["version", "-json"], environment)
  requireResult(version, "Terraform version check")
  if ((JSON.parse(version.stdout) as { terraform_version?: string }).terraform_version !== TERRAFORM_VERSION) {
    throw new CloudflareError(`Terraform ${TERRAFORM_VERSION} is required`)
  }
  const initialized = await runTerraform(["init", "-backend=false", "-input=false"], environment)
  requireResult(initialized, "Terraform validation initialization")
  const validated = await runTerraform(["validate", "-no-color"], environment)
  requireResult(validated, "Terraform validation")
}
