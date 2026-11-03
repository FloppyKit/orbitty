// Raycast-format script headers — the pure TS mirror of the Rust scan in
// `src-tauri/src/companion/script_cmds.rs`. The dock gets its rows from
// that scan; this parser keeps the same table (both `#` and `//`
// comments, first value wins) so qa can pin the grammar without a
// running app. Discovery never executes anything.

export const HEADER_BYTES = 4096;
export const MAX_FILES = 200;
export const MODES = ["silent", "compact", "fullOutput", "inline"] as const;
export type ScriptMode = (typeof MODES)[number];

export type ScriptArgument = {
  kind: "text" | "password";
  placeholder?: string;
  optional: boolean;
};

/** One row's worth of a script command. `id` is the absolute script path. */
export type ScriptCommand = {
  id: string;
  title: string;
  subtitle?: string;
  /** Emoji only in v1; a file path or URL is ignored. */
  icon?: string;
  mode: ScriptMode;
  argument1?: ScriptArgument;
  needsConfirmation: boolean;
  cwd?: string;
};

/** `# @raycast.key value` or `// @raycast.key value`. First value wins. */
export function headerMap(text: string): Map<string, string> {
  const out = new Map<string, string>();
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trimStart();
    const rest = line.startsWith("//")
      ? line.slice(2)
      : line.startsWith("#")
        ? line.slice(1)
        : null;
    if (rest === null) continue;
    const body = rest.trimStart();
    if (!body.startsWith("@raycast.")) continue;
    const meta = body.slice("@raycast.".length);
    const match = /^(\S+)\s*(.*)$/.exec(meta);
    if (!match) continue;
    const key = match[1] ?? "";
    if (!key || out.has(key)) continue;
    out.set(key, (match[2] ?? "").trim());
  }
  return out;
}

export function parseArgument(raw: string): ScriptArgument | null {
  let value: unknown;
  try {
    value = JSON.parse(raw);
  } catch {
    return null;
  }
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const obj = value as Record<string, unknown>;
  const kind = typeof obj.type === "string" ? obj.type.toLowerCase() : "";
  if (kind !== "text" && kind !== "password") return null;
  return {
    kind,
    placeholder:
      typeof obj.placeholder === "string" && obj.placeholder.trim()
        ? obj.placeholder.trim()
        : undefined,
    optional: obj.optional === true,
  };
}

/** Emoji only: a non-ASCII first token with no path separators. */
export function emojiOnly(raw: string): string | undefined {
  const token = raw.trim().split(/\s+/)[0] ?? "";
  if (!token || token.includes("/") || token.includes("\\")) return undefined;
  const first = token.codePointAt(0) ?? 0;
  if (first < 0x80) return undefined;
  return token;
}

/** Names the scan never opens, same list as the Rust side. */
export function skipName(name: string): string | null {
  if (name.startsWith(".")) return "hidden";
  if (name.includes(".template.")) return "template";
  return null;
}

export function isExecutableMode(mode: number): boolean {
  return (mode & 0o111) !== 0;
}

export type HeaderResult = { cmd: ScriptCommand } | { skip: string };

/**
 * Header text → row. `id` here is the file name; the Rust scan fills in
 * the absolute path. Malformed files are skipped, never thrown.
 */
export function parseScriptHeader(
  text: string,
  file: { name: string; folder: string },
): HeaderResult {
  const named = skipName(file.name);
  if (named) return { skip: named };
  const header = headerMap(text);
  if (header.get("schemaVersion") !== "1") return { skip: "schemaVersion" };
  const title = (header.get("title") ?? "").trim();
  if (!title) return { skip: "title" };
  const mode = (header.get("mode") ?? "").trim();
  if (!MODES.includes(mode as ScriptMode)) return { skip: "mode" };
  const platform = (header.get("platform") ?? "").toLowerCase();
  if (platform && !platform.includes("macos")) return { skip: "platform" };
  const packageName = (header.get("packageName") ?? "").trim();
  const description = (header.get("description") ?? "").trim();
  const cwd = (header.get("currentDirectoryPath") ?? "").trim();
  const argument = header.has("argument1")
    ? parseArgument(header.get("argument1") ?? "")
    : null;
  return {
    cmd: {
      id: file.name,
      title,
      subtitle: packageName || description || file.folder || undefined,
      icon: emojiOnly(header.get("icon") ?? ""),
      mode: mode as ScriptMode,
      argument1: argument ?? undefined,
      needsConfirmation:
        (header.get("needsConfirmation") ?? "").trim().toLowerCase() === "true",
      cwd: cwd || undefined,
    },
  };
}
