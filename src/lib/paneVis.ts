// GO-ORBITTY-HITCH-DRAW — native hide/show emits `orbitty:pane-vis`
// (`{ label, visible }`). Panes keep warm on hide, so a hidden webview's
// `document.hidden` never flips and its timers would keep firing into the
// void; pages gate those timers on this event instead.
import { listen } from "@tauri-apps/api/event";

export interface PaneVis {
  label: string;
  visible: boolean;
}

// Listen for one pane's visibility flips. Returns the unlisten fn.
export function listenPaneVis(
  label: string,
  onVis: (visible: boolean) => void,
): Promise<() => void> {
  return listen<PaneVis>("orbitty:pane-vis", (e) => {
    if (e.payload?.label === label) onVis(e.payload.visible);
  });
}
