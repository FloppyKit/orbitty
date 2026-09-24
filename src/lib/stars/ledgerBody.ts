import type { MetricFlags } from "../persist";

export interface LedgerSpoke {
  label: string;
  value: number;
}

export interface LedgerTrackerLine {
  planet: string;
  label: string;
  value: string;
}

export interface LedgerBits {
  dayKey: string;
  scopeLabel: string;
  flags: MetricFlags;
  spokes: readonly LedgerSpoke[];
  wins: readonly string[];
  grateful: string;
  trackers: readonly LedgerTrackerLine[];
}

function section(title: string, lines: string[]): string[] {
  return ["", title, ...lines];
}

// Local template only. A checked section with nothing filled still gets one
// honest line so the saved page is never a blank shell.
export function ledgerBody(bits: LedgerBits): string {
  const flags = bits.flags;
  const on =
    flags.spokes || flags.wins || flags.grateful || flags.trackers || flags.cloud;
  if (!on) return "";

  const lines: string[] = [`${bits.dayKey} · ${bits.scopeLabel}`];

  if (flags.spokes) {
    const set = bits.spokes.filter((s) => s.value >= 1);
    lines.push(
      ...section(
        "Spokes",
        set.length
          ? set.map((s) => `${s.label} ${Math.round(s.value)}`)
          : ["None set today."],
      ),
    );
  }

  if (flags.wins) {
    const wins = bits.wins.map((w) => w.trim()).filter(Boolean);
    lines.push(
      ...section("Wins", wins.length ? wins.map((w) => w) : ["None written."]),
    );
  }

  if (flags.grateful) {
    const g = bits.grateful.trim();
    lines.push(...section("Grateful", [g || "Nothing noted."]));
  }

  if (flags.trackers) {
    const rows = bits.trackers
      .map((t) => {
        const label = t.label.trim();
        const planet = t.planet.trim();
        if (!label) return "";
        return planet ? `${planet} · ${label} ${t.value}` : `${label} ${t.value}`;
      })
      .filter(Boolean);
    lines.push(...section("Trackers", rows.length ? rows : ["None yet."]));
  }

  if (flags.cloud) {
    lines.push(
      ...section("Cloud", ["The sky stays on the cover of this page."]),
    );
  }

  return lines.join("\n").trim();
}
