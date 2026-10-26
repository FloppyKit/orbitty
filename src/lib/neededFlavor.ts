// Same classify table as NeededRuleRow / needed_rules.rs. No model.

const GO_RE = /\b(GO-[A-Z0-9][-A-Z0-9]*)\b/;

export function extractGo(text: string): string | null {
  const m = GO_RE.exec((text || "").toUpperCase().replace(/\?/g, " "));
  return m ? m[1] : null;
}

export function whyGo(why: string): string {
  for (const tok of (why || "").split(/\s+/)) {
    if (tok.startsWith("go=")) {
      const v = tok.slice(3).trim();
      if (v) return v.toUpperCase();
    }
  }
  return "";
}

export function classifyNeeded(title: string, why: string) {
  const t = (title || "").trim();
  const low = t.toLowerCase();
  const go = extractGo(t) || whyGo(why) || null;
  let flavor = "fact";
  let promptKind: string | null = null;
  if (t.startsWith("Dogfood ") && go) {
    flavor = "dogfood";
    if (low.includes("leftover")) promptKind = "leftover";
  } else if (t.startsWith("Writer for ") && t.endsWith("?")) {
    if (low.includes("leftover")) {
      flavor = "writer-leftover";
      promptKind = "leftover";
    } else {
      flavor = "writer";
      promptKind = "first";
    }
  }
  return { go, flavor, promptKind };
}

export function showPassFail(
  kind: string | undefined,
  state: string | undefined,
  title: string,
  why: string,
): boolean {
  return (
    kind === "needed" &&
    state === "open" &&
    classifyNeeded(title, why).flavor === "dogfood"
  );
}

/** Fail will not save on "FAIL:" alone. */
export function failHasText(text: string): boolean {
  const t = (text || "").trim();
  if (!t.startsWith("FAIL:")) return false;
  return t.slice(5).trim().length > 0;
}

export function tip8From(title: string): string {
  const m = /\b([0-9a-f]{8})\b/i.exec(title || "");
  return m ? m[1] : "";
}

export function fixTitle(go: string, tip: string): string {
  return tip ? `Fix ${go} ${tip}?` : `Fix ${go}?`;
}
