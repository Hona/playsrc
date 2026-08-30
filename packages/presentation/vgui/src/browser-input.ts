/** Browser/OS shortcuts and IME delivery are not Source keyboard input.
 * Escape remains routable; cancelling it cannot suppress pointer-lock exit.
 * Clipboard chords are deliberately left to the focused text control. */
export function browserOwnsKey(event: Pick<KeyboardEvent, "code" | "metaKey" | "ctrlKey" | "altKey" | "isComposing" | "keyCode">): boolean {
  if (event.isComposing || event.keyCode === 229) return true
  if (event.metaKey) return !["KeyA", "KeyC", "KeyV", "KeyX"].includes(event.code)
  if (event.altKey && ["Tab", "F4", "ArrowLeft", "ArrowRight", "Home"].includes(event.code)) return true
  return event.ctrlKey && ["Tab", "PageUp", "PageDown", "KeyT", "KeyW", "KeyN", "KeyL", "KeyR", "KeyF", "KeyP", "KeyS", "KeyO", "KeyJ", "KeyH", "KeyU", "Equal", "Minus", "Digit0"].includes(event.code)
}

/** Native editing/selection menus stay available; Source widgets own the rest. */
export function vguiTextTarget(target: EventTarget | null, host: HTMLElement): boolean {
  for (let element = target as HTMLElement | null; element && element !== host; element = element.parentElement) {
    if (["INPUT", "TEXTAREA"].includes(element.tagName) || element.isContentEditable
      || ["textbox", "document"].includes(element.getAttribute?.("role") ?? "")) return true
  }
  return false
}
