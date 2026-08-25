import { render } from "preact"
import { useEffect, useRef } from "preact/hooks"
import { tf2StartupLoadingLabel } from "@playsrc/game-tf2-browser/startup-presentation"
import { ApplicationPublication } from "./application-publication"
import { Tf2Application, type ApplicationView } from "./runtime"
import "./style.css"

const initial: ApplicationView = Object.freeze({
  phase: "Startup",
  gameUi: "main-menu",
  detail: "Loading configured TF2 interface resources",
  pointerLocked: false,
  consoleVisible: false,
  blockers: Object.freeze([]),
  fireEvents: 0,
  explosionEvents: 0,
  bootstrapLoading: true,
  bootstrapProgress: 0,
})

function App() {
  const applicationRoot = useRef<HTMLElement>(null)
  const canvas = useRef<HTMLCanvasElement>(null)
  const vgui = useRef<HTMLDivElement>(null)
  const gameUi = useRef<HTMLDivElement>(null)
  const hud = useRef<HTMLDivElement>(null)
  const engineer = useRef<HTMLDivElement>(null)
  const classSelection = useRef<HTMLDivElement>(null)
  const teamSelection = useRef<HTMLDivElement>(null)
  const options = useRef<HTMLDivElement>(null)
  const loading = useRef<HTMLDivElement>(null)
  const startup = useRef<HTMLDivElement>(null)
  const startupVideo = useRef<HTMLVideoElement>(null)
  const startupLoading = useRef<HTMLDivElement>(null)
  const runtime = useRef<Tf2Application>()

  useEffect(() => {
    if (!applicationRoot.current || !canvas.current || !vgui.current || !gameUi.current || !hud.current || !engineer.current || !classSelection.current || !teamSelection.current
      || !options.current || !loading.current || !startup.current || !startupVideo.current || !startupLoading.current) return
    const publication = new ApplicationPublication({
      root: applicationRoot.current,
      canvas: canvas.current,
      loadingLabel: startupLoading.current,
    })
    publication.publish(initial)
    const application = new Tf2Application(canvas.current, {
      vgui: vgui.current,
      gameUi: gameUi.current,
      hud: hud.current,
      engineer: engineer.current,
      classSelection: classSelection.current,
      teamSelection: teamSelection.current,
      options: options.current,
      loading: loading.current,
      startup: startup.current,
      startupVideo: startupVideo.current,
    }, (view) => publication.publish(view))
    runtime.current = application
    void application.start()
    return () => {
      runtime.current = undefined
      void application.close()
    }
  }, [])

  return (
    <main
      ref={applicationRoot}
      class="tf2-application"
      data-phase={initial.phase}
      data-bootstrap-loading="true"
      data-bootstrap-progress="0"
      data-startup-muted-fallback="false"
      data-detail={initial.detail}
      data-gameui={initial.gameUi}
      data-gameplay-initialized="false"
      data-pointer-locked="false"
      data-console-visible="false"
      data-class-selection-visible="false"
      data-options-visible="false"
      data-team-selection-visible="false"
      data-fire-events="0"
      data-explosion-events="0"
      data-particle-items="0"
      data-projectiles="0"
      data-environment-drawables="0"
      data-blockers="[]"
    >
      <canvas
        ref={canvas}
        class="world-canvas"
        tabIndex={-1}
        aria-label="TF2 game view"
        aria-hidden="true"
        onClick={(event) => void runtime.current?.requestPointer(event.currentTarget)}
        onContextMenu={(event) => event.preventDefault()}
      />
      <div ref={startup} class="startup-layer" aria-label="Valve startup movie">
        <video
          ref={startupVideo}
          class="startup-movie"
          playsInline
          preload="auto"
          tabIndex={0}
          onClick={() => runtime.current?.admitStartupGesture()}
          onKeyDown={(event) => { if (event.code === "Escape") runtime.current?.startupKey(event.code) }}
        />
        <div ref={startupLoading} class="startup-loading-plaque" role="status" aria-live="polite">
          {tf2StartupLoadingLabel(initial.bootstrapProgress ?? 0)}
        </div>
      </div>
      <div ref={loading} class="vgui-layer loading-layer" aria-label="TF2 map loading" />
      <div ref={gameUi} class="vgui-layer gameui-layer" aria-label="TF2 GameUI" />
      <div ref={hud} class="vgui-layer hud-layer" aria-label="TF2 HUD" />
      <div ref={engineer} class="vgui-layer engineer-layer" aria-label="Engineer buildings" />
      <div ref={classSelection} class="vgui-layer class-selection-layer" aria-label="TF2 class selection" />
      <div ref={teamSelection} class="vgui-layer team-selection-layer" aria-label="TF2 team selection" />
      <div ref={options} class="vgui-layer options-layer" aria-label="TF2 Options" />
      <div ref={vgui} class="vgui-layer developer-layer" aria-label="TF2 developer interface" />
    </main>
  )
}

const root = document.getElementById("app")
if (!root) throw new Error("Application mount is missing")
render(<App />, root)
