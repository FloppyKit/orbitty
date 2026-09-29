import { invoke } from "@tauri-apps/api/core";

export interface LinkPreview {
  kind: string;
  target: string;
  name: string;
  host?: string | null;
  size?: number | null;
  modified?: string | null;
  modifiedSecs?: number | null;
  excerpt?: string | null;
  warning?: string | null;
  thumbnail?: string | null;
}

interface LinkClick {
  action: string;
  preview?: LinkPreview | null;
}

export const linkUi = $state({
  preview: null as LinkPreview | null,
  error: "",
  busy: false,
});

export function linkErrText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error && e.message) return e.message;
  return "couldn't open link";
}

/** First click. An exact approval opens in Rust. Otherwise the preview opens. */
export async function requestLink(url: string): Promise<void> {
  const res = await invoke<LinkClick>("click_link", { url });
  if (res.action === "preview" && res.preview) {
    linkUi.preview = res.preview;
    linkUi.error = "";
    linkUi.busy = false;
  } else {
    linkUi.preview = null;
    linkUi.error = "";
  }
}

export function dismissPreview() {
  linkUi.preview = null;
  linkUi.error = "";
  linkUi.busy = false;
}

export async function confirmOpen(): Promise<void> {
  const p = linkUi.preview;
  if (!p || linkUi.busy) return;
  linkUi.busy = true;
  linkUi.error = "";
  try {
    await invoke("open_link", { url: p.target });
    linkUi.preview = null;
  } catch (e) {
    const msg = linkErrText(e);
    linkUi.error = /file not found/i.test(msg) ? "file not found" : msg;
  } finally {
    linkUi.busy = false;
  }
}

export async function revealCurrent(): Promise<void> {
  const p = linkUi.preview;
  if (!p || linkUi.busy) return;
  linkUi.busy = true;
  linkUi.error = "";
  try {
    await invoke("reveal_link", { url: p.target });
  } catch (e) {
    linkUi.error = linkErrText(e);
  } finally {
    linkUi.busy = false;
  }
}
