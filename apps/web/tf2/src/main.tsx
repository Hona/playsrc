import { render } from "preact"
import { useEffect, useRef, useState } from "preact/hooks"
import { Tf2Application, type ApplicationView } from "./runtime"
import "./style.css"

const initial: ApplicationView = Object.freeze({
  phase: "Loading",
  detail: "Starting TF2 jump practice",
  pointerLocked: false,
  consoleVisible: false,
  blockers: Object.freeze([]),
  fireEvents: 0,
  explosionEvents: 0,
})

function App() {
  const canvas = useRef<HTMLCanvasElement>(null)
  const vgui = useRef<HTMLDivElement>(null)
  const runtime = useRef<Tf2Application>()
  const [view, setView] = useState<ApplicationView>(initial)
  const [showBlockers, setShowBlockers] = useState(false)

  useEffect(() => {
    if (!canvas.current || !vgui.current) return
    const application = new Tf2Application(canvas.current, vgui.current, setView)
    runtime.current = application
    void application.start()
    return () => {
      runtime.current = undefined
      void application.close()
    }
  }, [])

  const busy = view.phase === "Loading" || view.phase === "Replacing"
  return (
    <main
      class="field-shell"
      data-phase={view.phase}
      data-projectiles={view.hud?.projectileCount ?? 0}
      data-fire-events={view.fireEvents}
      data-explosion-events={view.explosionEvents}
    >
      <canvas
        ref={canvas}
        class="world-canvas"
        tabIndex={0}
        aria-label="TF2 jump practice world"
        onClick={() => void runtime.current?.requestPointer()}
        onContextMenu={(event) => event.preventDefault()}
      />

      <header class="field-header">
        <div class="wordmark" aria-label="playsrc TF2">
          <span class="wordmark-source">play</span><span class="wordmark-class">src</span>
          <small>jump practice / jump_beef</small>
        </div>
        <div class="status-stamp" data-ready={view.phase === "Ready"}>
          <b>{view.phase}</b>
          <span>{view.detail}</span>
        </div>
      </header>

      {view.hud && (
        <section class="hud" aria-label="Player status">
          <div class="health-readout">
            <span>health</span>
            <strong>{Math.max(0, Math.round(view.hud.health))}</strong>
            <i>/ {view.hud.maxHealth}</i>
          </div>
          <div class="loadout-readout">
            <b>{view.hud.className}</b>
            <span>{view.hud.weaponName}</span>
          </div>
          <div class="speed-readout">
            <span>speed</span>
            <strong>{Math.round(view.hud.speed)}</strong>
            <i>HU/s</i>
          </div>
        </section>
      )}

      <nav class="class-rail" aria-label="Class selection">
        <button type="button" onClick={() => runtime.current?.selectClass(1)}>
          <kbd>1</kbd><span>Soldier</span>
        </button>
        <button type="button" onClick={() => runtime.current?.selectClass(2)}>
          <kbd>2</kbd><span>Demoman</span>
        </button>
        <button type="button" class="console-toggle" onClick={() => runtime.current?.toggleConsole()}>
          <kbd>`</kbd><span>Console</span>
        </button>
        <button type="button" class="audio-toggle" onClick={() => void runtime.current?.resumeAudio()}>
          <kbd>♪</kbd><span>Audio</span>
        </button>
      </nav>

      <aside class="support-card">
        <button
          type="button"
          aria-expanded={showBlockers}
          onClick={() => setShowBlockers((value) => !value)}
        >
          <span>{view.blockers.length} exact support blockers</span>
          <b>{showBlockers ? "close" : "inspect"}</b>
        </button>
        {showBlockers && (
          <ol>
            {view.blockers.map((blocker) => <li key={blocker}>{blocker}</li>)}
          </ol>
        )}
      </aside>

      <footer class="field-footer">
        <span>{view.pointerLocked ? "mouse captured" : "click field to capture mouse"}</span>
        <span>WASD move · Space jump · Mouse 1 fire · Mouse 2 detonate</span>
        <span>derived cache {view.cache ?? "pending"}</span>
      </footer>

      {busy && (
        <div class="loading-plate" role="status" aria-live="polite">
          <div class="loader-mark" />
          <b>{view.phase}</b>
          <span>{view.detail}</span>
        </div>
      )}
      {view.phase === "Failed" && (
        <div class="failure-plate" role="alert">
          <b>Practice could not start</b>
          <span>{view.detail}</span>
          <small>Check the exact support list and restart the local owner.</small>
        </div>
      )}

      <div ref={vgui} class="vgui-mount" aria-label="VGUI layer" />
    </main>
  )
}

const root = document.getElementById("app")
if (!root) throw new Error("Application mount is missing")
render(<App />, root)
