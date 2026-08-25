(() => {
  const isolatedKeys = new Set([
    "KeyW", "KeyA", "KeyS", "KeyD", "Space",
    "ShiftLeft", "ShiftRight", "ControlLeft", "ControlRight",
  ])
  const state = { suppressedPointerEvents: 0, suppressedKeyboardEvents: 0 }
  Object.defineProperty(globalThis, "__playsrcBrowserAutomationInput", {
    configurable: true,
    value: state,
  })
  const isolate = (event) => {
    if (!event.isTrusted) return
    const main = document.querySelector("main")
    if (main?.dataset.phase !== "Ready" || main.dataset.gameui !== "in-game") return
    if (event instanceof KeyboardEvent) {
      if (!isolatedKeys.has(event.code)) return
      state.suppressedKeyboardEvents += 1
    } else {
      const canvas = document.querySelector("canvas.world-canvas")
      if (!canvas || document.pointerLockElement !== canvas) return
      state.suppressedPointerEvents += 1
    }
    event.preventDefault()
    event.stopImmediatePropagation()
  }
  for (const type of ["mousemove", "mousedown", "mouseup", "keydown", "keyup"]) {
    window.addEventListener(type, isolate, { capture: true })
  }
})()
