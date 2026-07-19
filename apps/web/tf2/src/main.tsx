import { render } from "preact"
import { useEffect, useRef, useState } from "preact/hooks"
import { Tf2Application, type ApplicationView } from "./runtime"
import "./style.css"

const initial: ApplicationView = Object.freeze({
  phase: "Loading",
  gameUi: "main-menu",
  detail: "Loading configured TF2 interface resources",
  pointerLocked: false,
  consoleVisible: false,
  blockers: Object.freeze([]),
  fireEvents: 0,
  explosionEvents: 0,
})

function App() {
  const canvas = useRef<HTMLCanvasElement>(null)
  const vgui = useRef<HTMLDivElement>(null)
  const gameUi = useRef<HTMLDivElement>(null)
  const hud = useRef<HTMLDivElement>(null)
  const options = useRef<HTMLDivElement>(null)
  const runtime = useRef<Tf2Application>()
  const [view, setView] = useState<ApplicationView>(initial)

  useEffect(() => {
    if (!canvas.current || !vgui.current || !gameUi.current || !hud.current || !options.current) return
    const application = new Tf2Application(canvas.current, {
      vgui: vgui.current,
      gameUi: gameUi.current,
      hud: hud.current,
      options: options.current,
    }, setView)
    runtime.current = application
    void application.start()
    return () => {
      runtime.current = undefined
      void application.close()
    }
  }, [])

  return (
    <main
      class="tf2-application"
      data-phase={view.phase}
      data-detail={view.detail}
      data-gameui={view.gameUi}
      data-gameplay-initialized={view.snapshotTick === undefined ? "false" : "true"}
      data-pointer-locked={view.pointerLocked ? "true" : "false"}
      data-hud-probe={view.hudProbe}
      data-hud-animation-trace={view.hudAnimationTrace}
      data-hud-operation-probe={view.hudOperationProbe}
      data-options-visible={view.optionsVisible ? "true" : "false"}
      data-settings-persistence={view.settingsPersistence}
      data-settings-apply={view.settingsApply}
      data-cache={view.cache}
      data-host-request={view.hostRequest}
      data-presentation-random-state={view.presentationRandomState}
      data-presentation-character={view.presentationCharacter}
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
      data-projectiles={view.projectileStates ? view.projectileStates.split(",").length : 0}
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
      data-environment={view.environment ? `${view.environment.profile},${view.environment.clusters},${view.environment.skySurfaces},${view.environment.waterVolumes},${view.environment.marks},${view.environment.markFragments}` : undefined}
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
      data-water-plan={view.waterPlanProbe}
      data-water-passes={view.waterPasses?.join(",")}
      data-water-restored={view.waterStateRestored}
      data-water-normal-frame={view.waterNormalFrame}
      data-reload-history={view.reloadHistory?.join("|")}
      data-fire-ticks={view.fireTickHistory?.join("|")}
      data-performance={view.performanceProbe}
      data-locker={view.lockerProbe}
      data-unsupported-state={view.unsupportedState}
      data-blockers={JSON.stringify(view.blockers)}
    >
      <canvas
        ref={canvas}
        class="world-canvas"
        tabIndex={view.gameUi === "in-game" ? 0 : -1}
        aria-label="TF2 game view"
        aria-hidden={view.gameUi === "main-menu" ? "true" : "false"}
        onClick={(event) => void runtime.current?.requestPointer(event.currentTarget)}
        onContextMenu={(event) => event.preventDefault()}
      />
      <div ref={gameUi} class="vgui-layer gameui-layer" aria-label="TF2 GameUI" />
      <div ref={hud} class="vgui-layer hud-layer" aria-label="TF2 HUD" />
      <div ref={options} class="vgui-layer options-layer" aria-label="TF2 Options" />
      <div ref={vgui} class="vgui-layer developer-layer" aria-label="TF2 developer interface" />
    </main>
  )
}

const root = document.getElementById("app")
if (!root) throw new Error("Application mount is missing")
render(<App />, root)
