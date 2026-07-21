export type ApplicationPresentationViewport = Readonly<{ width: number; height: number; devicePixelRatio: number; revision: number }>

type ListenerTarget = Readonly<{
  addEventListener(type: string, listener: () => void): void
  removeEventListener(type: string, listener: () => void): void
}>

export type PresentationViewportPlatform = Readonly<{
  measure(): Readonly<{ width: number; height: number; devicePixelRatio: number }>
  requestFrame(callback: () => void): number
  cancelFrame(handle: number): void
  observeResize(callback: () => void): Readonly<{ disconnect(): void }>
  visualViewport: ListenerTarget | null
  orientation: ListenerTarget | null
  document: ListenerTarget
  resolutionQuery(devicePixelRatio: number): ListenerTarget
}>

export type PresentationViewportOwnerSnapshot = Readonly<{
  lifecycle: "live" | "destroyed"
  viewport: ApplicationPresentationViewport | null
  notifications: number
  measurements: number
  publications: number
  suspensions: number
  pendingFrames: number
  listeners: number
  observers: number
}>

export type PresentationViewportOwner = Readonly<{
  first(): Promise<ApplicationPresentationViewport>
  notify(): void
  snapshot(): PresentationViewportOwnerSnapshot
  destroy(): void
}>

export function initializePresentationViewportOwner(request: Readonly<{
  platform: PresentationViewportPlatform
  onViewport(viewport: ApplicationPresentationViewport): void
  onSuspended(): void
}>): PresentationViewportOwner {
  let lifecycle: "live" | "destroyed" = "live"
  let viewport: ApplicationPresentationViewport | null = null
  let revision = 0
  let notifications = 0
  let measurements = 0
  let publications = 0
  let suspensions = 0
  let suspended = false
  let frame: number | null = null
  let resolutionQuery: ListenerTarget | null = null
  let resolutionDevicePixelRatio: number | null = null
  let firstSettled = false
  let resolveFirst!: (value: ApplicationPresentationViewport) => void
  let rejectFirst!: (reason: Error) => void
  const first = new Promise<ApplicationPresentationViewport>((resolve, reject) => {
    resolveFirst = resolve
    rejectFirst = reject
  })

  const same = (left: ApplicationPresentationViewport | null, right: Readonly<{ width: number; height: number; devicePixelRatio: number }>): boolean =>
    left !== null
    && left.width === right.width
    && left.height === right.height
    && left.devicePixelRatio === right.devicePixelRatio

  const schedule = (): void => {
    if (lifecycle === "destroyed") return
    notifications += 1
    if (frame !== null) return
    frame = request.platform.requestFrame(commit)
  }

  const resolutionChanged = (): void => schedule()

  const armResolutionQuery = (devicePixelRatio: number): void => {
    if (resolutionDevicePixelRatio === devicePixelRatio) return
    resolutionQuery?.removeEventListener("change", resolutionChanged)
    resolutionQuery = request.platform.resolutionQuery(devicePixelRatio)
    resolutionDevicePixelRatio = devicePixelRatio
    resolutionQuery.addEventListener("change", resolutionChanged)
  }

  function commit(): void {
    frame = null
    if (lifecycle === "destroyed") return
    measurements += 1
    const measured = request.platform.measure()
    const width = Math.trunc(measured.width)
    const height = Math.trunc(measured.height)
    const devicePixelRatio = measured.devicePixelRatio
    const valid = Number.isFinite(width) && Number.isFinite(height)
      && width > 0 && width <= 32767 && height > 0 && height <= 32767
      && Number.isFinite(devicePixelRatio) && devicePixelRatio >= 0.5 && devicePixelRatio <= 8
    if (!valid) {
      if (!suspended) {
        viewport = null
        suspended = true
        suspensions += 1
        request.onSuspended()
      }
      return
    }
    suspended = false
    armResolutionQuery(devicePixelRatio)
    if (same(viewport, { width, height, devicePixelRatio })) return
    revision += 1
    viewport = Object.freeze({ width, height, devicePixelRatio, revision })
    publications += 1
    request.onViewport(viewport)
    if (!firstSettled) {
      firstSettled = true
      resolveFirst(viewport)
    }
  }

  const resizeObserver = request.platform.observeResize(schedule)
  request.platform.visualViewport?.addEventListener("resize", schedule)
  request.platform.orientation?.addEventListener("change", schedule)
  request.platform.document.addEventListener("fullscreenchange", schedule)
  request.platform.document.addEventListener("pointerlockchange", schedule)
  schedule()

  return Object.freeze({
    first: () => first,
    notify: schedule,
    snapshot: () => Object.freeze({
      lifecycle,
      viewport,
      notifications,
      measurements,
      publications,
      suspensions,
      pendingFrames: frame === null ? 0 : 1,
      listeners: lifecycle === "destroyed" ? 0 : 2 + Number(request.platform.visualViewport !== null) + Number(request.platform.orientation !== null) + Number(resolutionQuery !== null),
      observers: lifecycle === "destroyed" ? 0 : 1,
    }),
    destroy: () => {
      if (lifecycle === "destroyed") return
      lifecycle = "destroyed"
      if (frame !== null) request.platform.cancelFrame(frame)
      frame = null
      resizeObserver.disconnect()
      request.platform.visualViewport?.removeEventListener("resize", schedule)
      request.platform.orientation?.removeEventListener("change", schedule)
      request.platform.document.removeEventListener("fullscreenchange", schedule)
      request.platform.document.removeEventListener("pointerlockchange", schedule)
      resolutionQuery?.removeEventListener("change", resolutionChanged)
      resolutionQuery = null
      if (!firstSettled) {
        firstSettled = true
        rejectFirst(new Error("Presentation viewport owner was destroyed before a positive content box was admitted"))
      }
    },
  })
}

export function initializeBrowserPresentationViewportOwner(request: Readonly<{
  root: HTMLElement
  onViewport(viewport: ApplicationPresentationViewport): void
  onSuspended(): void
}>): PresentationViewportOwner {
  const owner = initializePresentationViewportOwner({
    platform: {
      measure: () => Object.freeze({
        width: request.root.clientWidth,
        height: request.root.clientHeight,
        devicePixelRatio: window.devicePixelRatio,
      }),
      requestFrame: (callback) => requestAnimationFrame(callback),
      cancelFrame: (handle) => cancelAnimationFrame(handle),
      observeResize: (callback) => {
        const observer = new ResizeObserver(callback)
        observer.observe(request.root, { box: "content-box" })
        return Object.freeze({ disconnect: () => observer.disconnect() })
      },
      visualViewport: window.visualViewport,
      orientation: screen.orientation ?? null,
      document,
      resolutionQuery: (devicePixelRatio) => matchMedia(`(resolution: ${devicePixelRatio}dppx)`),
    },
    onViewport: (viewport) => {
      request.root.dataset.presentationViewportState = "active"
      request.root.dataset.presentationViewport = `${viewport.width}x${viewport.height}@${viewport.devicePixelRatio}`
      request.root.dataset.presentationViewportRevision = String(viewport.revision)
      request.onViewport(viewport)
    },
    onSuspended: () => {
      request.root.dataset.presentationViewportState = "suspended"
      delete request.root.dataset.presentationViewport
      request.onSuspended()
    },
  })
  return owner
}
