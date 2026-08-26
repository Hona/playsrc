import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import type { FullResult, Reporter, TestCase, TestResult } from "@playwright/test/reporter"

type TimedTest = Readonly<{
  title: string
  status: string
  durationMilliseconds: number
  startedAtMilliseconds: number
  phases: unknown
}>

export default class WallClockReporter implements Reporter {
  readonly #created = Date.now()
  #began = this.#created
  readonly #tests: TimedTest[] = []

  onBegin(): void {
    this.#began = Date.now()
  }

  onTestEnd(test: TestCase, result: TestResult): void {
    const attachment = result.attachments.find((entry) => entry.name === "profile-wall-clock-phases")
    let phases: unknown = null
    if (attachment?.body) {
      try { phases = JSON.parse(attachment.body.toString("utf8")) } catch { phases = null }
    }
    this.#tests.push(Object.freeze({
      title: test.title,
      status: result.status,
      durationMilliseconds: result.duration,
      startedAtMilliseconds: result.startTime.getTime(),
      phases,
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
    })
    await mkdir(path.dirname(destination), { recursive: true })
    await writeFile(destination, `${JSON.stringify(report, null, 2)}\n`)
  }
}
