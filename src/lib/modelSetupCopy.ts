/** Alpha model-setup copy. No Svelte. node:test imports this file. */

export const LMSTUDIO_OFF =
  "LM Studio is installed, but its server is off. In LM Studio choose Developer > Start Server, port 1234, then Look again.";

export const HARNESS_GROUP = "Agent harnesses";

export const HARNESS_BLURB = "These run a planet row. They are not this chat's model.";

export const HARNESSES: { id: string; line: string }[] = [
  { id: "grok", line: "Signed-in Grok CLI for a planet row, not this chat." },
  { id: "goose", line: "Goose CLI on this Mac for a planet row, not this chat." },
  { id: "cursor", line: "Cursor agent. A signed-in harness for a planet row, not this chat." },
  { id: "hermes", line: "Hermes opens your existing chat app for a planet row, not this chat." },
  { id: "pi", line: "Pi coding agent. Runs in Terminal for a planet row, not this chat." },
];

export function harnessLine(id: string): string {
  return HARNESSES.find((h) => h.id === id)?.line ?? "Agent harness for a planet row, not this chat.";
}
