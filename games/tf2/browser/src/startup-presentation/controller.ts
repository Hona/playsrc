import { TF2_CONFIGURED_STARTUP, type Tf2StartupDescriptor } from "./descriptor"

export type Tf2StartupState =
  | Readonly<{ kind: "NotStarted" }>
  | Readonly<{ kind: "Preparing" }>
  | Readonly<{ kind: "AwaitingGesture" }>
  | Readonly<{ kind: "Playing" }>
  | Readonly<{ kind: "WaitingForMenu"; movieResult: "Skipped" | "Completed" }>
  | Readonly<{ kind: "Skipped"; reason: "Escape" | "Policy" }>
  | Readonly<{ kind: "Completed" }>
  | Readonly<{ kind: "Failed"; stage: "MediaPreparation" | "Playback" | "MenuPreparation"; reason: string }>
  | Readonly<{ kind: "Destroyed" }>

export type Tf2StartupPolicy = Readonly<{
  benchmark: boolean
  editMode: boolean
  forceVr: boolean
  developer: boolean
  noVideo: boolean
  allowDebug: boolean
  healthWarningPresent: boolean
}>

export type Tf2StartupMediaSession = Readonly<{
  play(): Promise<"started" | "gesture-required">
  admitGesture(): Promise<"started">
  skip(): void
  setVisible(visible: boolean): void
  destroy(): void
}>

export type Tf2HiddenMenu = Readonly<{ reveal(): void; destroy(): void }>

export type Tf2StartupControllerRequest = Readonly<{
  descriptor?: Tf2StartupDescriptor
  policy: Tf2StartupPolicy
  media: Readonly<{
    prepare(descriptor: Tf2StartupDescriptor, events: Readonly<{ completed(): void; failed(reason: string): void }>): Promise<Tf2StartupMediaSession>
  }>
  menu: Readonly<{ prepareHidden(): Promise<Tf2HiddenMenu> }>
  clock: Readonly<{ nowMicroseconds(): number }>
  onState?(state: Tf2StartupState): void
}>

export type Tf2StartupController = Readonly<{
  state(): Tf2StartupState
  transitionTimeMicroseconds(): number
  start(): void
  gesture(): void
  key(key: string): void
  visibility(visible: boolean): void
  destroy(): void
}>

const frozen = <T extends Tf2StartupState>(value: T): T => Object.freeze(value)

function suppressed(policy: Tf2StartupPolicy): boolean {
  return policy.benchmark || policy.editMode || policy.forceVr
    || (!policy.healthWarningPresent && (policy.developer || policy.noVideo || policy.allowDebug))
}

export function createTf2StartupController(request: Tf2StartupControllerRequest): Tf2StartupController {
  let current: Tf2StartupState = frozen({ kind: "NotStarted" })
  let transitionTime = request.clock.nowMicroseconds()
  let generation = 0
  let started = false
  let terminal: "Escape" | "Policy" | "Completed" | null = null
  let media: Tf2StartupMediaSession | null = null
  let menu: Tf2HiddenMenu | null = null
  let mediaDestroyed = false
  let menuDestroyed = false

  const publish = (state: Tf2StartupState): void => {
    current = state
    transitionTime = request.clock.nowMicroseconds()
    request.onState?.(state)
  }
  const fail = (stage: Extract<Tf2StartupState, { kind: "Failed" }>["stage"], reason: unknown): void => {
    if (current.kind === "Destroyed" || current.kind === "Failed") return
    const text = reason instanceof Error ? reason.message : String(reason)
    publish(frozen({ kind: "Failed", stage, reason: text }))
  }
  const releaseMedia = (): void => {
    if (media && !mediaDestroyed) { mediaDestroyed = true; media.destroy() }
  }
  const revealIfReady = (): void => {
    if (!terminal || !menu || current.kind === "Destroyed" || current.kind === "Failed") return
    releaseMedia()
    menu.reveal()
    publish(terminal === "Completed" ? frozen({ kind: "Completed" }) : frozen({ kind: "Skipped", reason: terminal }))
  }
  const finishMovie = (result: "Escape" | "Completed"): void => {
    if (terminal || current.kind === "Destroyed" || current.kind === "Failed") return
    terminal = result
    if (!menu) publish(frozen({ kind: "WaitingForMenu", movieResult: result === "Completed" ? "Completed" : "Skipped" }))
    revealIfReady()
  }

  return Object.freeze({
    state: () => current,
    transitionTimeMicroseconds: () => transitionTime,
    start: () => {
      if (started || current.kind === "Destroyed") return
      started = true
      const ownGeneration = ++generation
      publish(frozen({ kind: "Preparing" }))
      void request.menu.prepareHidden().then((prepared) => {
        if (ownGeneration !== generation) { prepared.destroy(); return }
        menu = prepared
        revealIfReady()
      }, (reason) => { if (ownGeneration === generation) fail("MenuPreparation", reason) })
      if (suppressed(request.policy)) {
        terminal = "Policy"
        if (!menu) publish(frozen({ kind: "WaitingForMenu", movieResult: "Skipped" }))
        revealIfReady()
        return
      }
      void request.media.prepare(request.descriptor ?? TF2_CONFIGURED_STARTUP, Object.freeze({
        completed: () => { if (ownGeneration === generation) finishMovie("Completed") },
        failed: (reason) => { if (ownGeneration === generation) fail("Playback", reason) },
      })).then(async (prepared) => {
        if (ownGeneration !== generation) { prepared.destroy(); return }
        media = prepared
        try {
          const result = await prepared.play()
          if (ownGeneration !== generation || terminal) return
          publish(result === "started" ? frozen({ kind: "Playing" }) : frozen({ kind: "AwaitingGesture" }))
        } catch (reason) { if (ownGeneration === generation) fail("Playback", reason) }
      }, (reason) => { if (ownGeneration === generation) fail("MediaPreparation", reason) })
    },
    gesture: () => {
      if (current.kind !== "AwaitingGesture" || !media) return
      const ownGeneration = generation
      void media.admitGesture().then(() => {
        if (ownGeneration === generation && current.kind === "AwaitingGesture") publish(frozen({ kind: "Playing" }))
      }, (reason) => { if (ownGeneration === generation) fail("Playback", reason) })
    },
    key: (key) => {
      if (key !== "Escape" || (current.kind !== "Playing" && current.kind !== "AwaitingGesture") || !media) return
      media.skip()
      finishMovie("Escape")
    },
    visibility: (visible) => { if (current.kind === "Playing" || current.kind === "AwaitingGesture") media?.setVisible(visible) },
    destroy: () => {
      if (current.kind === "Destroyed") return
      generation += 1
      releaseMedia()
      if (menu && !menuDestroyed) { menuDestroyed = true; menu.destroy() }
      publish(frozen({ kind: "Destroyed" }))
    },
  })
}
