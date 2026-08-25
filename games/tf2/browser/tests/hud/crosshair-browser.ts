import {
  bindTf2Hud,
  tf2HudAvailable,
  tf2HudUnavailable,
  type Tf2HudBinding,
  type Tf2HudCrosshair,
  type Tf2HudPlayer,
  type Tf2HudSnapshot,
  type Tf2HudWeapon,
} from "../../src/hud"
import { Tf2HudCrosshairPresentation } from "../../src/hud-integration/crosshair"

const root = document.getElementById("tf2-crosshair-headed-root")
if (!root) throw new Error("TF2 headed crosshair root is unavailable")
const presentation = new Tf2HudCrosshairPresentation(root)
let tick = 0n

function binding(
  style: string,
  color: readonly [number, number, number],
  scale: number,
  weaponScale: number,
  visible: boolean,
): Tf2HudBinding {
  const weapon: Tf2HudWeapon = Object.freeze({
    identity: 1,
    itemDefinition: tf2HudUnavailable("not-produced"),
    displayName: "Rocket Launcher",
    slot: 0,
    position: 0,
    selectable: true,
    ammoDisplay: "clip-and-reserve",
    clip: tf2HudAvailable(4),
    reserve: tf2HudAvailable(20),
    maximumClip: tf2HudAvailable(4),
    maximumReserve: tf2HudAvailable(20),
    reload: "ready",
    drawsCrosshair: true,
  })
  const crosshair: Tf2HudCrosshair = Object.freeze({
    configured: visible,
    weaponAllows: true,
    loadingImage: false,
    paused: false,
    clientModeAllows: true,
    frozen: false,
    localViewEntity: true,
    vguiInput: false,
    observerMode: "none",
    observerCrosshair: true,
    tfSuppressed: false,
    countdownHidden: false,
    texture: style === "" ? "crosshair_default" : `vgui/crosshairs/${style}`,
    color: Object.freeze([color[0], color[1], color[2], 255]),
    scale,
    weaponScale,
  })
  const player: Tf2HudPlayer = Object.freeze({
    identity: 1,
    lifecycle: "active",
    class: tf2HudAvailable(3),
    team: tf2HudAvailable(2),
    playerClassUsePlayerModel: false,
    classModel: tf2HudAvailable(Object.freeze({ identity: "models/player/soldier.mdl", skin: 0 })),
    health: tf2HudAvailable(Object.freeze({ current: 200, maximum: 200, maximumBuffed: 300 })),
    conditions: Object.freeze([0, 0, 0, 0, 0]),
    weapons: Object.freeze([weapon]),
    activeWeapon: tf2HudAvailable(1),
    weaponSelection: Object.freeze({ open: false, selectedWeapon: tf2HudUnavailable("not-applicable") }),
    crosshair: tf2HudAvailable(crosshair),
    liveHudSuppressed: false,
    respawnAllowed: false,
  })
  const snapshot: Tf2HudSnapshot = Object.freeze({
    tick: ++tick,
    player: tf2HudAvailable(player),
    scoreboard: tf2HudUnavailable("not-produced"),
    freezePanel: tf2HudUnavailable("not-produced"),
  })
  return bindTf2Hud(Object.freeze({
    previous: tf2HudUnavailable("replay-discontinuity"),
    snapshot,
    events: Object.freeze([]),
  }))
}

Object.assign(window, {
  __playsrcTf2Crosshair: Object.freeze({
    show(
      style: string,
      color: readonly [number, number, number],
      scale: number,
      weaponScale = 1,
      visible = true,
    ) {
      presentation.publish(binding(style, color, scale, weaponScale, visible), {
        width: window.innerWidth,
        height: window.innerHeight,
      })
      const element = document.querySelector<HTMLElement>('[data-tf2-crosshair="authored"]')
      if (!element) throw new Error("TF2 authored crosshair element is unavailable")
      const bounds = element.getBoundingClientRect()
      return {
        display: getComputedStyle(element).display,
        left: bounds.left,
        top: bounds.top,
        width: bounds.width,
        height: bounds.height,
        material: element.dataset.sourceMaterial,
        texture: element.dataset.sourceTexture,
        style: element.dataset.crosshairStyle,
      }
    },
  }),
})
