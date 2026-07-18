export const VGUI_CSS = `
.playsrc-vgui-root {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  overflow: hidden;
  pointer-events: none;
  contain: layout style;
}

.playsrc-vgui-root[data-vgui-service="developer-console"] {
  z-index: 1;
}

.playsrc-vgui-root[data-vgui-service="client-diagnostics"] {
  z-index: 0;
}

.playsrc-vgui-control {
  box-sizing: border-box;
}

.playsrc-vgui-frame {
  position: absolute;
  overflow: visible;
  pointer-events: auto;
  background: var(--vgui-frame-background-unfocused);
  border: 0;
  color: var(--vgui-input-text);
  touch-action: none;
  transition: background-color var(--vgui-frame-focus-transition) linear;
  user-select: none;
}

.playsrc-vgui-frame[data-focused="true"],
.playsrc-vgui-frame[data-interaction] {
  background: var(--vgui-frame-background);
}

.playsrc-vgui-title-background,
.playsrc-vgui-titlebar,
.playsrc-vgui-title,
.playsrc-vgui-close,
.playsrc-vgui-client,
.playsrc-vgui-history,
.playsrc-vgui-entry,
.playsrc-vgui-submit,
.playsrc-vgui-completion {
  position: absolute;
}

.playsrc-vgui-title-background {
  overflow: hidden;
  background: var(--vgui-title-background-unfocused);
  transition: background-color var(--vgui-frame-focus-transition) linear;
  z-index: 0;
}

.playsrc-vgui-frame[data-focused="true"] .playsrc-vgui-title-background,
.playsrc-vgui-frame[data-interaction] .playsrc-vgui-title-background {
  background: var(--vgui-title-background);
}

.playsrc-vgui-titlebar {
  z-index: 2;
  cursor: move;
  background: transparent;
}

.playsrc-vgui-title {
  overflow: hidden;
  color: var(--vgui-title-text-unfocused);
  z-index: 1;
  font-family: var(--vgui-title-font);
  font-size: var(--vgui-title-size);
  font-style: var(--vgui-title-style);
  font-weight: var(--vgui-title-weight);
  font-synthesis: none;
  line-height: var(--vgui-title-line-height);
  white-space: nowrap;
  pointer-events: none;
  transition: color var(--vgui-frame-focus-transition) linear;
}

.playsrc-vgui-frame[data-focused="true"] .playsrc-vgui-title,
.playsrc-vgui-frame[data-interaction] .playsrc-vgui-title {
  color: var(--vgui-title-text);
}

.playsrc-vgui-close {
  appearance: none;
  padding: 0;
  cursor: default;
  background: transparent;
  border: 0;
  color: var(--vgui-close-unfocused);
  z-index: 5;
  transition: color var(--vgui-frame-focus-transition) linear;
}

.playsrc-vgui-frame[data-focused="true"] .playsrc-vgui-close,
.playsrc-vgui-frame[data-interaction] .playsrc-vgui-close {
  color: var(--vgui-close);
}

.playsrc-vgui-close::before,
.playsrc-vgui-close::after {
  position: absolute;
  left: 48%;
  top: 50%;
  width: 1px;
  height: var(--vgui-close-glyph-size);
  background: currentColor;
  content: "";
  transform: translateY(-50%) rotate(45deg);
  transform-origin: center;
}

.playsrc-vgui-close::after {
  transform: translateY(-50%) rotate(-45deg);
}

.playsrc-vgui-grip {
  position: absolute;
  z-index: 4;
  background: transparent;
}

.playsrc-vgui-grip-n,
.playsrc-vgui-grip-s { cursor: ns-resize; }
.playsrc-vgui-grip-e,
.playsrc-vgui-grip-w { cursor: ew-resize; }
.playsrc-vgui-grip-ne,
.playsrc-vgui-grip-sw { cursor: nesw-resize; }
.playsrc-vgui-grip-nw,
.playsrc-vgui-grip-se { cursor: nwse-resize; }

.playsrc-vgui-client {
  overflow: visible;
}

.playsrc-vgui-history {
  overflow: auto;
  margin: 0;
  background: var(--vgui-history-background);
  border-color: var(--vgui-history-border-top) var(--vgui-history-border-right) var(--vgui-history-border-bottom) var(--vgui-history-border-left);
  border-style: solid;
  border-width: var(--vgui-history-border-width-top) var(--vgui-history-border-width-right) var(--vgui-history-border-width-bottom) var(--vgui-history-border-width-left);
  color: var(--vgui-normal-output);
  font-family: var(--vgui-console-font);
  font-size: var(--vgui-console-size);
  font-style: var(--vgui-console-style);
  font-weight: var(--vgui-console-weight);
  font-synthesis: none;
  line-height: var(--vgui-console-line-height);
  overflow-wrap: anywhere;
  padding: calc(var(--vgui-history-inset-top) + var(--vgui-history-draw-y)) var(--vgui-history-inset-right) var(--vgui-history-inset-bottom) calc(var(--vgui-history-inset-left) + var(--vgui-history-draw-x));
  user-select: text;
  white-space: pre-wrap;
}

.playsrc-vgui-output-segment {
  white-space: pre-wrap;
}

.playsrc-vgui-entry {
  min-width: 0;
  margin: 0;
  padding: calc(var(--vgui-entry-inset-top) + var(--vgui-entry-draw-y)) calc(var(--vgui-entry-inset-right) + var(--vgui-entry-draw-x)) calc(var(--vgui-entry-inset-bottom) + var(--vgui-entry-draw-y)) calc(var(--vgui-entry-inset-left) + var(--vgui-entry-draw-x));
  background: var(--vgui-input-background);
  border-color: var(--vgui-entry-border-top) var(--vgui-entry-border-right) var(--vgui-entry-border-bottom) var(--vgui-entry-border-left);
  border-style: solid;
  border-width: var(--vgui-entry-border-width-top) var(--vgui-entry-border-width-right) var(--vgui-entry-border-width-bottom) var(--vgui-entry-border-width-left);
  border-radius: 0;
  caret-color: var(--vgui-input-cursor);
  color: var(--vgui-input-text);
  font-family: var(--vgui-entry-font);
  font-size: var(--vgui-entry-size);
  font-style: var(--vgui-entry-style);
  font-weight: var(--vgui-entry-weight);
  font-synthesis: none;
  line-height: var(--vgui-entry-line-height);
  outline: 0;
  user-select: text;
}

.playsrc-vgui-entry::selection {
  background: var(--vgui-input-selection-background);
  color: var(--vgui-input-selection-text);
}

.playsrc-vgui-entry:focus {
  box-shadow: inset 0 0 0 1px var(--vgui-focus);
}

.playsrc-vgui-submit {
  appearance: none;
  margin: 0;
  padding: var(--vgui-submit-inset-top) var(--vgui-submit-inset-right) var(--vgui-submit-inset-bottom) var(--vgui-submit-inset-left);
  background: var(--vgui-submit-background);
  border-color: var(--vgui-submit-border-top) var(--vgui-submit-border-right) var(--vgui-submit-border-bottom) var(--vgui-submit-border-left);
  border-style: solid;
  border-width: var(--vgui-submit-border-width-top) var(--vgui-submit-border-width-right) var(--vgui-submit-border-width-bottom) var(--vgui-submit-border-width-left);
  border-radius: 0;
  color: var(--vgui-submit-text);
  font-family: var(--vgui-submit-font);
  font-size: var(--vgui-submit-size);
  font-style: var(--vgui-submit-style);
  font-weight: var(--vgui-submit-weight);
  font-synthesis: none;
  line-height: var(--vgui-submit-line-height);
  outline: 0;
}

.playsrc-vgui-submit:hover,
.playsrc-vgui-submit:focus-visible {
  background: var(--vgui-submit-armed-background);
  color: var(--vgui-submit-armed-text);
}

.playsrc-vgui-submit:active {
  background: var(--vgui-submit-depressed-background);
  border-color: var(--vgui-submit-depressed-border-top) var(--vgui-submit-depressed-border-right) var(--vgui-submit-depressed-border-bottom) var(--vgui-submit-depressed-border-left);
  border-width: var(--vgui-submit-depressed-border-width-top) var(--vgui-submit-depressed-border-width-right) var(--vgui-submit-depressed-border-width-bottom) var(--vgui-submit-depressed-border-width-left);
  color: var(--vgui-submit-depressed-text);
  padding: var(--vgui-submit-depressed-inset-top) var(--vgui-submit-depressed-inset-right) var(--vgui-submit-depressed-inset-bottom) var(--vgui-submit-depressed-inset-left);
}

.playsrc-vgui-completion {
  overflow: hidden auto;
  z-index: 6;
  margin: 0;
  padding: 0;
  background: var(--vgui-completion-background);
  border-color: var(--vgui-completion-border-top) var(--vgui-completion-border-right) var(--vgui-completion-border-bottom) var(--vgui-completion-border-left);
  border-style: solid;
  border-width: var(--vgui-completion-border-width-top) var(--vgui-completion-border-width-right) var(--vgui-completion-border-width-bottom) var(--vgui-completion-border-width-left);
  color: var(--vgui-completion-text);
  font-family: var(--vgui-completion-font);
  font-size: var(--vgui-completion-size);
  font-style: var(--vgui-completion-style);
  font-weight: var(--vgui-completion-weight);
  font-synthesis: none;
  line-height: var(--vgui-completion-line-height);
  list-style: none;
}

.playsrc-vgui-menu-item {
  overflow: hidden;
  padding: 0 0 0 var(--vgui-completion-text-inset);
  color: var(--vgui-completion-text);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.playsrc-vgui-menu-item:hover,
.playsrc-vgui-menu-item[aria-selected="true"] {
  background: var(--vgui-completion-armed-background);
  color: var(--vgui-completion-armed-text);
}

.playsrc-vgui-menu-overflow {
  cursor: default;
}

.playsrc-vgui-diagnostics {
  position: absolute;
  overflow: hidden;
  z-index: 0;
  pointer-events: none;
  background: transparent;
  font-family: var(--vgui-diagnostic-font);
  font-size: var(--vgui-diagnostic-size);
  font-style: var(--vgui-diagnostic-style);
  font-weight: var(--vgui-diagnostic-weight);
  font-synthesis: none;
  line-height: var(--vgui-diagnostic-line-height);
  text-align: left;
  text-shadow: var(--vgui-diagnostic-shadow);
  white-space: pre;
}

.playsrc-vgui-diagnostic-line {
  position: absolute;
  left: var(--vgui-diagnostic-padding);
  right: 0;
  overflow: hidden;
  text-overflow: clip;
  white-space: pre;
}

.playsrc-vgui-root[data-platform-font-capability="unsupported"] .playsrc-vgui-title,
.playsrc-vgui-root[data-platform-font-capability="unsupported"] .playsrc-vgui-history,
.playsrc-vgui-root[data-platform-font-capability="unsupported"] .playsrc-vgui-output-segment,
.playsrc-vgui-root[data-platform-font-capability="unsupported"] .playsrc-vgui-entry,
.playsrc-vgui-root[data-platform-font-capability="unsupported"] .playsrc-vgui-submit,
.playsrc-vgui-root[data-platform-font-capability="unsupported"] .playsrc-vgui-completion,
.playsrc-vgui-root[data-platform-font-capability="unsupported"] .playsrc-vgui-menu-item,
.playsrc-vgui-root[data-platform-font-capability="unsupported"] .playsrc-vgui-diagnostic-line {
  color: transparent !important;
  text-shadow: none !important;
  -webkit-text-fill-color: transparent !important;
}

.playsrc-vgui-root[data-reduced-motion="true"] *,
.playsrc-vgui-root[data-reduced-motion="true"] *::before,
.playsrc-vgui-root[data-reduced-motion="true"] *::after {
  animation: none !important;
  scroll-behavior: auto !important;
  transition: none !important;
}

`
