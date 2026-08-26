import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import type { FullResult, Reporter, TestCase, TestResult, TestStep } from "@playwright/test/reporter"

type TimedTest = Readonly<{
  title: string
  status: string
  durationMilliseconds: number
  startedAtMilliseconds: number
  phases: unknown
  operations: unknown
}>

export default class WallClockReporter implements Reporter {
  readonly #created = Date.now()
  #began = this.#created
  readonly #tests: TimedTest[] = []
  readonly #fixtures: Array<{ title: string; durationMilliseconds: number; error: string | null }> = []

  onStepEnd(_test: TestCase, _result: TestResult, step: TestStep): void {
    if (step.category === "fixture") this.#fixtures.push({ title: step.title, durationMilliseconds: step.duration, error: step.error?.message ?? null })
  }

  onBegin(): void {
    this.#began = Date.now()
  }

  onTestEnd(test: TestCase, result: TestResult): void {
    const attachment = result.attachments.find((entry) => entry.name === "profile-wall-clock-phases")
    let phases: unknown = null
    let operations: unknown = null
    if (attachment?.body) {
      try { phases = JSON.parse(attachment.body.toString("utf8")) } catch { phases = null }
    }
    const operationPhases = result.attachments.find(entry => entry.name === "profile-operation-phases")
    if (operationPhases?.body) {
      try { operations = JSON.parse(operationPhases.body.toString("utf8")) } catch { operations = null }
    }
    this.#tests.push(Object.freeze({
      title: test.title,
      status: result.status,
      durationMilliseconds: result.duration,
      startedAtMilliseconds: result.startTime.getTime(),
      phases,
      operations,
    }))
  }

  async onEnd(result: FullResult): Promise<void> {
    const destination = process.env.PLAYSRC_PROFILE_TIMING_PATH
    if (!destination) return
    const finished = Date.now()
    const processStarted = Number(process.env.PLAYSRC_PROFILE_PROCESS_STARTED ?? this.#created)
    const report = Object.freeze({
      schema: "playsrc-headed-playwright-wall-clock-v1",
      status: result.status,
      processMilliseconds: finished - processStarted,
      runnerInitializationMilliseconds: this.#began - processStarted,
      testsMilliseconds: this.#tests.reduce((total, test) => total + test.durationMilliseconds, 0),
      teardownMilliseconds: Math.max(0, finished - Math.max(this.#began, ...this.#tests.map((test) => test.startedAtMilliseconds + test.durationMilliseconds))),
      tests: this.#tests,
      fixtures: this.#fixtures,
    })
    await mkdir(path.dirname(destination), { recursive: true })
    await writeFile(destination, `${JSON.stringify(report, null, 2)}\n`)
  }
}
