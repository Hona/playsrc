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
      data-camera-position={view.camera?.position.join(",")}
      data-camera-yaw={view.camera?.yawDegrees}
      data-camera-pitch={view.camera?.pitchDegrees}
      data-camera-vertical-fov={view.camera?.verticalFovDegrees}
      data-camera-near={view.camera?.near}
      data-camera-far={view.camera?.far}
      data-spawn-entity={view.initialView?.entity}
      data-spawn-hammer-id={view.initialView?.hammerId}
      data-spawn-position={view.initialView?.position.join(",")}
      data-spawn-angles={view.initialView?.angles.join(",")}
      data-particle-items={view.particleRenderItems ?? 0}
      data-crouch-fraction={view.movement?.crouchFraction}
      data-view-offset={view.movement?.viewOffset.join(",")}
      data-movement-mode={view.movementTick?.mode}
      data-wish-speed={view.movementTick?.wishSpeed}
      data-climbed-step={view.movementTick?.climbedStep}
      data-viewmodel-activity={view.viewmodelPose?.activity}
      data-viewmodel-sequence={view.viewmodelPose?.sequence}
      data-viewmodel-cycle={view.viewmodelPose?.cycle}
      data-viewmodel-primitives={view.viewmodelPose?.primitives}
      data-model-probes={view.modelProbes?.map((probe) => `${probe.model}:${probe.sequence}:${probe.primitives}:${probe.vertices}`).join("|")}
      data-audio-voices={view.audioVoices?.join(",")}
      data-snapshot-tick={view.snapshotTick}
      data-projectile-states={view.projectileStates}
      data-decal-probe={view.decalProbe}
      data-model-occurrences={view.modelOccurrenceCount}
      data-particle-probe={view.particleProbe}
      data-audio-starts={view.audioStarts?.join("|")}
      data-viewmodel-projection={view.viewmodelProjection}
      data-viewmodel-activities={view.viewmodelActivities?.join(",")}
      data-viewmodel-sequences={view.viewmodelSequences}
      data-crouch-history={view.crouchHistory?.join("|")}
      data-grounded={view.movement?.grounded}
      data-viewmodel-timelines={view.viewmodelTimelineProbes?.join("|")}
      data-environment={
        view.environment
          ? `${view.environment.profile},${view.environment.clusters},${view.environment.skySurfaces},${view.environment.waterVolumes},${view.environment.marks},${view.environment.markFragments}`
          : undefined
      }
      data-environment-drawables={view.environmentDrawables ?? 0}
      data-visible-decal-fragments={view.visibleDecalFragments}
      data-environment-sky={view.environment?.sky?.name}
      data-water-cubemap={view.environment?.waterVolumeFacts[0]?.cubemapSample ?? undefined}
      data-viewmodel-depth-range={view.viewmodelDepthRange}
      data-viewmodel-viewport-restored={view.viewmodelViewportRestored}
      data-model-matrices={view.modelMatrices ? JSON.stringify(view.modelMatrices) : undefined}
      data-decal-state={view.decalStateProbe ? JSON.stringify(view.decalStateProbe) : undefined}
      data-weapon-trace={view.weaponTrace}
      data-authority-trace={view.authorityTrace}
      data-entity-trace={view.entityTrace}
      data-model-material-probe={view.modelMaterialProbe}
      data-random-audio-probe={view.randomAudioProbe}
      data-collision-mover-probe={view.collisionMoverProbe}
      data-simulation-probe={view.simulationProbe}
      data-brush-model-probe={view.brushModelProbe}
      data-unsupported-state={view.unsupportedState}
      data-blockers={JSON.stringify(view.blockers)}
    >
      <canvas
        ref={canvas}
        class="world-canvas"
        tabIndex={0}
        aria-label="TF2 jump practice world"
        onClick={(event) => void runtime.current?.requestPointer(event.currentTarget)}
        onContextMenu={(event) => event.preventDefault()}
      />

      <header class="field-header">
        <div class="wordmark" aria-label="playsrc TF2">
          <span class="wordmark-source">play</span>
          <span class="wordmark-class">src</span>
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
          <kbd>1</kbd>
          <span>Soldier</span>
        </button>
        <button type="button" onClick={() => runtime.current?.selectClass(2)}>
          <kbd>2</kbd>
          <span>Demoman</span>
        </button>
        <button type="button" class="console-toggle" onClick={() => runtime.current?.toggleConsole()}>
          <kbd>`</kbd>
          <span>Console</span>
        </button>
        <button type="button" class="audio-toggle" onClick={() => void runtime.current?.resumeAudio()}>
          <kbd>♪</kbd>
          <span>Audio</span>
        </button>
      </nav>

      <aside class="support-card">
        <button type="button" aria-expanded={showBlockers} onClick={() => setShowBlockers((value) => !value)}>
          <span>{view.blockers.length} exact support blockers</span>
          <b>{showBlockers ? "close" : "inspect"}</b>
        </button>
        {showBlockers && (
          <ol>
            {view.blockers.map((blocker) => (
              <li key={blocker}>{blocker}</li>
            ))}
          </ol>
        )}
      </aside>

      <footer class="field-footer">
        <span>{view.pointerLocked ? "mouse captured" : "click field to capture mouse"}</span>
          <span>WASD move · Space jump · Shift crouch · R reload · Mouse 1 fire · Mouse 2 detonate</span>
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
