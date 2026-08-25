import { tf2StartupLoadingLabel } from "@playsrc/game-tf2-browser/startup-presentation"
import type { ApplicationView } from "./runtime"

type PublicationAttributeTarget = Readonly<{
  getAttribute(name: string): string | null
  setAttribute(name: string, value: string): void
  removeAttribute(name: string): void
}>

type PublicationCanvas = PublicationAttributeTarget & { tabIndex: number }
type PublicationLabel = { textContent: string | null }
type PublicationValue = string | number | boolean | null | undefined

type PublicationBinding = Readonly<{
  attribute: string
  dependency: keyof ApplicationView
  value(view: ApplicationView): PublicationValue
}>

const attribute = (
  name: string,
  dependency: keyof ApplicationView,
  value: (view: ApplicationView) => PublicationValue = (view) => view[dependency] as PublicationValue,
): PublicationBinding => Object.freeze({
  attribute: `data-${name}`,
  dependency,
  value,
})

const attributes: readonly PublicationBinding[] = Object.freeze([
  attribute("phase", "phase"),
  attribute("generation", "generation"),
  attribute("startup-state", "startupState"),
  attribute("startup-gestures", "startupGestures"),
  attribute("menu-preparation", "menuPreparation"),
  attribute("bootstrap-loading", "bootstrapLoading", (view) => Boolean(view.bootstrapLoading)),
  attribute("bootstrap-progress", "bootstrapProgress", (view) => view.bootstrapProgress ?? 0),
  attribute("startup-muted-fallback", "startupMutedFallback", (view) => Boolean(view.startupMutedFallback)),
  attribute("loading-progress", "loadingProgress"),
  attribute("loading-status", "loadingStatus"),
  attribute("loading-background", "loadingBackground"),
  attribute("detail", "detail"),
  attribute("gameui", "gameUi"),
  attribute("gameplay-initialized", "snapshotTick", (view) => view.snapshotTick !== undefined),
  attribute("pointer-locked", "pointerLocked"),
  attribute("console-visible", "consoleVisible"),
  attribute("class-selection-visible", "classSelectionVisible", (view) => Boolean(view.classSelectionVisible)),
  attribute("class-selection-team", "classSelectionTeam"),
  attribute("class-selection-selected", "classSelectionSelected"),
  attribute("class-selection-models", "classSelectionModels"),
  attribute("hud-probe", "hudProbe"),
  attribute("hud-animation-trace", "hudAnimationTrace"),
  attribute("hud-operation-probe", "hudOperationProbe"),
  attribute("hud-presentation-probe", "hudPresentationProbe"),
  attribute("scoreboard-visible", "scoreboardVisible", (view) => Boolean(view.scoreboardVisible)),
  attribute("scoreboard-probe", "scoreboardProbe"),
  attribute("options-visible", "optionsVisible", (view) => Boolean(view.optionsVisible)),
  attribute("local-match-visible", "localMatchVisible", (view) => Boolean(view.localMatchVisible)),
  attribute("local-match-entry", "localMatchEntry"),
  attribute("local-match-settings", "localMatchSettings"),
  attribute("team-selection-visible", "teamSelectionVisible", (view) => Boolean(view.teamSelectionVisible)),
  attribute("team-selection-local", "teamSelectionLocal"),
  attribute("team-selection-red-count", "teamSelectionRedCount"),
  attribute("team-selection-blue-count", "teamSelectionBlueCount"),
  attribute("team-selection-models", "teamSelectionModels"),
  attribute("settings-persistence", "settingsPersistence"),
  attribute("settings-apply", "settingsApply"),
  attribute("cache", "cache"),
  attribute("host-request", "hostRequest"),
  attribute("presentation-random-state", "presentationRandomState"),
  attribute("presentation-character", "presentationCharacter"),
  attribute("fire-events", "fireEvents"),
  attribute("explosion-events", "explosionEvents"),
  attribute("camera-position", "camera", (view) => view.camera?.position.join(",")),
  attribute("camera-yaw", "camera", (view) => view.camera?.yawDegrees),
  attribute("camera-pitch", "camera", (view) => view.camera?.pitchDegrees),
  attribute("camera-vertical-fov", "camera", (view) => view.camera?.verticalFovDegrees),
  attribute("camera-near", "camera", (view) => view.camera?.near),
  attribute("camera-far", "camera", (view) => view.camera?.far),
  attribute("pointer-movement", "pointerMovement"),
  attribute("display-frame", "displayFrame"),
  attribute("display-view-revision", "displayViewRevision"),
  attribute("display-prepared-revision", "displayPreparedRevision"),
  attribute("spawn-entity", "initialView", (view) => view.initialView?.entity),
  attribute("spawn-hammer-id", "initialView", (view) => view.initialView?.hammerId),
  attribute("spawn-position", "initialView", (view) => view.initialView?.position.join(",")),
  attribute("spawn-angles", "initialView", (view) => view.initialView?.angles.join(",")),
  attribute("particle-items", "particleRenderItems", (view) => view.particleRenderItems ?? 0),
  attribute("flame-points", "flamePoints", (view) => view.flamePoints ?? 0),
  attribute("projectiles", "projectileStates", (view) => view.projectileStates ? view.projectileStates.split(",").length : 0),
  attribute("crouch-fraction", "movement", (view) => view.movement?.crouchFraction),
  attribute("water-level", "movement", (view) => view.movement?.waterLevel),
  attribute("water-type", "movement", (view) => view.movement?.waterType),
  attribute("player-flags", "playerFlags"),
  attribute("in-water", "inWater"),
  attribute("view-offset", "movement", (view) => view.movement?.viewOffset.join(",")),
  attribute("movement-mode", "movementTick", (view) => view.movementTick?.mode),
  attribute("wish-speed", "movementTick", (view) => view.movementTick?.wishSpeed),
  attribute("climbed-step", "movementTick", (view) => view.movementTick?.climbedStep),
  attribute("sweep-queries", "movementTick", (view) => view.movementTick?.sweepQueries),
  attribute("point-queries", "movementTick", (view) => view.movementTick?.pointQueries),
  attribute("movement-contacts", "movementTick", (view) => view.movementTick?.contacts),
  attribute("movement-events", "movementTick", (view) => view.movementTick?.events),
  attribute("viewmodel-activity", "viewmodelPose", (view) => view.viewmodelPose?.activity),
  attribute("viewmodel-sequence", "viewmodelPose", (view) => view.viewmodelPose?.sequence),
  attribute("viewmodel-cycle", "viewmodelPose", (view) => view.viewmodelPose?.cycle),
  attribute("viewmodel-primitives", "viewmodelPose", (view) => view.viewmodelPose?.primitives),
  attribute("model-probes", "modelProbes", (view) => view.modelProbes?.map((probe) => `${probe.model}:${probe.sequence}:${probe.primitives}:${probe.vertices}`).join("|")),
  attribute("audio-voices", "audioVoices", (view) => view.audioVoices?.join(",")),
  attribute("snapshot-tick", "snapshotTick"),
  attribute("projectile-states", "projectileStates"),
  attribute("decal-probe", "decalProbe"),
  attribute("model-occurrences", "modelOccurrenceCount"),
  attribute("particle-probe", "particleProbe"),
  attribute("audio-starts", "audioStarts", (view) => view.audioStarts?.join("|")),
  attribute("viewmodel-projection", "viewmodelProjection"),
  attribute("viewmodel-activities", "viewmodelActivities", (view) => view.viewmodelActivities?.join(",")),
  attribute("viewmodel-sequences", "viewmodelSequences"),
  attribute("crouch-history", "crouchHistory", (view) => view.crouchHistory?.join("|")),
  attribute("grounded", "movement", (view) => view.movement?.grounded),
  attribute("vertical-speed", "movement", (view) => view.movement?.velocity[2]),
  attribute("viewmodel-timelines", "viewmodelTimelineProbes", (view) => view.viewmodelTimelineProbes?.join("|")),
  attribute("environment", "environment", (view) => view.environment ? `${view.environment.profile},${view.environment.clusters},${view.environment.skySurfaces},${view.environment.waterVolumes},${view.environment.marks},${view.environment.markFragments}` : undefined),
  attribute("environment-drawables", "environmentDrawables", (view) => view.environmentDrawables ?? 0),
  attribute("visible-decal-fragments", "visibleDecalFragments"),
  attribute("environment-sky", "environment", (view) => view.environment?.sky?.name),
  attribute("water-cubemap", "environment", (view) => view.environment?.waterVolumeFacts[0]?.cubemapSample ?? undefined),
  attribute("viewmodel-depth-range", "viewmodelDepthRange"),
  attribute("viewmodel-viewport-restored", "viewmodelViewportRestored"),
  attribute("viewmodel-world-depth-isolated", "viewmodelWorldDepthIsolated"),
  attribute("model-matrices", "modelMatrices", (view) => view.modelMatrices ? JSON.stringify(view.modelMatrices) : undefined),
  attribute("decal-state", "decalStateProbe", (view) => view.decalStateProbe ? JSON.stringify(view.decalStateProbe) : undefined),
  attribute("weapon-trace", "weaponTrace"),
  attribute("spy-probe", "spyProbe"),
  attribute("spy-watch-activity", "spyWatchActivity"),
  attribute("authority-trace", "authorityTrace"),
  attribute("entity-trace", "entityTrace"),
  attribute("model-material-probe", "modelMaterialProbe"),
  attribute("random-audio-probe", "randomAudioProbe"),
  attribute("collision-mover-probe", "collisionMoverProbe"),
  attribute("simulation-probe", "simulationProbe"),
  attribute("brush-model-probe", "brushModelProbe"),
  attribute("water-plan", "waterPlanProbe"),
  attribute("water-passes", "waterPasses", (view) => view.waterPasses?.join(",")),
  attribute("water-restored", "waterStateRestored"),
  attribute("water-normal-frame", "waterNormalFrame"),
  attribute("world-material-frames", "worldMaterialFrames"),
  attribute("reload-history", "reloadHistory", (view) => view.reloadHistory?.join("|")),
  attribute("fire-ticks", "fireTickHistory", (view) => view.fireTickHistory?.join("|")),
  attribute("performance", "performanceProbe"),
  attribute("performance-detail", "performanceDetailProbe"),
  attribute("load-performance", "loadPerformanceProbe"),
  attribute("locker", "lockerProbe"),
  attribute("ctf", "objectiveProbe"),
  attribute("ctf-events", "objectiveEventProbe"),
  attribute("round-probe", "roundProbe"),
  attribute("bot-count", "botCount"),
  attribute("bot-probe", "botProbe"),
  attribute("pickup-count", "pickupCount"),
  attribute("pickup-probe", "pickupProbe"),
  attribute("metal", "metal"),
  attribute("building-count", "buildingCount"),
  attribute("building-probe", "buildingProbe"),
  attribute("engineer-metal", "engineerMetal"),
  attribute("engineer-menu", "engineerMenu"),
  attribute("placement", "placementProbe"),
  attribute("unsupported-state", "unsupportedState"),
  attribute("blockers", "blockers", (view) => JSON.stringify(view.blockers)),
])

export type ApplicationPublicationTargets = Readonly<{
  root: PublicationAttributeTarget
  canvas: PublicationCanvas
  loadingLabel: PublicationLabel
}>

export class ApplicationPublication {
  readonly #targets: ApplicationPublicationTargets
  #previous?: ApplicationView

  constructor(targets: ApplicationPublicationTargets) {
    this.#targets = targets
  }

  publish(view: ApplicationView): void {
    const previous = this.#previous
    for (const binding of attributes) {
      if (previous && Object.is(previous[binding.dependency], view[binding.dependency])) continue
      const next = binding.value(view)
      const current = this.#targets.root.getAttribute(binding.attribute)
      if (next === undefined || next === null) {
        if (current !== null) this.#targets.root.removeAttribute(binding.attribute)
      } else {
        const encoded = String(next)
        if (current !== encoded) this.#targets.root.setAttribute(binding.attribute, encoded)
      }
    }
    if (!previous || previous.gameUi !== view.gameUi) {
      const tabIndex = view.gameUi === "in-game" ? 0 : -1
      if (this.#targets.canvas.tabIndex !== tabIndex) this.#targets.canvas.tabIndex = tabIndex
      const hidden = String(view.gameUi === "main-menu")
      if (this.#targets.canvas.getAttribute("aria-hidden") !== hidden) this.#targets.canvas.setAttribute("aria-hidden", hidden)
    }
    if (!previous || previous.bootstrapProgress !== view.bootstrapProgress) {
      const label = tf2StartupLoadingLabel(view.bootstrapProgress ?? 0)
      if (this.#targets.loadingLabel.textContent !== label) this.#targets.loadingLabel.textContent = label
    }
    this.#previous = view
  }
}
