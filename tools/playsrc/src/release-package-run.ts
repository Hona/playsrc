/** Only successful main-branch package builds from this repository are admissible. */
export function assertReleasePackageRun(value: unknown, sha: string, repository: string): void {
  const run = value as { head_sha?: unknown; head_branch?: unknown; path?: unknown; event?: unknown; status?: unknown; conclusion?: unknown; repository?: { full_name?: unknown } } | null
  if (!/^[0-9a-f]{40}$/.test(sha) || !run || run.head_sha !== sha || run.head_branch !== "main"
    || run.path !== ".github/workflows/prepare-release.yml" || run.event !== "workflow_dispatch"
    || run.status !== "completed" || run.conclusion !== "success" || run.repository?.full_name !== repository) {
    throw new Error("Prepared release artifact must come from a successful Prepare release package run on this repository's exact main commit")
  }
}

if (import.meta.main) assertReleasePackageRun(JSON.parse(await Bun.stdin.text()), process.argv[2], process.argv[3])
