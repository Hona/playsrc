export const VGUI_CONSOLE_CSS = `
.playsrc-vgui-root {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
  pointer-events: none;
  contain: layout style;
}

.playsrc-vgui-control {
  box-sizing: border-box;
}

.playsrc-vgui-frame {
  position: absolute;
  overflow: visible;
  pointer-events: auto;
  background: var(--vgui-frame-background);
  border-color: var(--vgui-border-color);
  border-style: var(--vgui-border-style);
  border-width: var(--vgui-border-width);
  color: var(--vgui-input-text);
}

.playsrc-vgui-title {
  position: absolute;
  inset: 0 0 auto 0;
  overflow: hidden;
  color: var(--vgui-title-text);
  font-family: var(--vgui-title-font);
  font-size: var(--vgui-title-size);
  font-style: var(--vgui-title-style);
  font-weight: var(--vgui-title-weight);
  line-height: var(--vgui-title-line-height);
  white-space: nowrap;
}

.playsrc-vgui-client,
.playsrc-vgui-history,
.playsrc-vgui-entry,
.playsrc-vgui-submit,
.playsrc-vgui-completion {
  position: absolute;
}

.playsrc-vgui-client {
  overflow: visible;
}

.playsrc-vgui-history {
  overflow: auto;
  margin: 0;
  padding: 0;
  background: var(--vgui-history-background);
  border-color: var(--vgui-border-color);
  border-style: var(--vgui-border-style);
  border-width: var(--vgui-border-width);
  color: var(--vgui-normal-output);
  font-family: var(--vgui-console-font);
  font-size: var(--vgui-console-size);
  font-style: var(--vgui-console-style);
  font-weight: var(--vgui-console-weight);
  line-height: var(--vgui-console-line-height);
  overflow-wrap: anywhere;
  white-space: pre-wrap;
}

.playsrc-vgui-output-segment {
  white-space: pre-wrap;
}

.playsrc-vgui-entry {
  min-width: 0;
  padding: 0;
  background: var(--vgui-input-background);
  border-color: var(--vgui-border-color);
  border-style: var(--vgui-border-style);
  border-width: var(--vgui-border-width);
  color: var(--vgui-input-text);
  font-family: var(--vgui-console-font);
  font-size: var(--vgui-console-size);
  font-style: var(--vgui-console-style);
  font-weight: var(--vgui-console-weight);
  line-height: var(--vgui-console-line-height);
}

.playsrc-vgui-submit {
  padding: 0;
  background: var(--vgui-frame-background);
  border-color: var(--vgui-border-color);
  border-style: var(--vgui-border-style);
  border-width: var(--vgui-border-width);
  color: var(--vgui-input-text);
  font-family: var(--vgui-completion-font);
  font-size: var(--vgui-completion-size);
  font-style: var(--vgui-completion-style);
  font-weight: var(--vgui-completion-weight);
  line-height: var(--vgui-completion-line-height);
}

.playsrc-vgui-completion {
  overflow: hidden auto;
  z-index: 1;
  margin: 0;
  padding: 0;
  background: var(--vgui-completion-background);
  border-color: var(--vgui-border-color);
  border-style: var(--vgui-border-style);
  border-width: var(--vgui-border-width);
  color: var(--vgui-completion-text);
  font-family: var(--vgui-completion-font);
  font-size: var(--vgui-completion-size);
  font-style: var(--vgui-completion-style);
  font-weight: var(--vgui-completion-weight);
  line-height: var(--vgui-completion-line-height);
  list-style: none;
}

.playsrc-vgui-menu-item {
  overflow: hidden;
  padding: 0;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.playsrc-vgui-menu-item[aria-selected="true"] {
  background: var(--vgui-completion-selected);
}

.playsrc-vgui-menu-overflow {
  cursor: default;
}

.playsrc-vgui-entry:focus-visible,
.playsrc-vgui-submit:focus-visible {
  outline-color: var(--vgui-focus);
  outline-offset: 1px;
  outline-style: solid;
  outline-width: 1px;
}

.playsrc-vgui-root[data-reduced-motion="true"] *,
.playsrc-vgui-root[data-reduced-motion="true"] *::before,
.playsrc-vgui-root[data-reduced-motion="true"] *::after {
  animation: none !important;
  scroll-behavior: auto !important;
  transition: none !important;
}
`
