// Rail-only reader of the seat door. The habitat sees the word on the snapshot.

export type SeatStates = ReadonlyMap<string, string>;

const FLOOR_MS = 5000;

function isRecord(v: unknown): v is Record<string, unknown> {
  return !!v && typeof v === "object" && !Array.isArray(v);
}

export function parseDoor(v: unknown): SeatStates {
  if (!isRecord(v) || v.enabled !== true || !Array.isArray(v.seats)) return new Map();
  const out = new Map<string, string>();
  for (const row of v.seats) {
    if (!isRecord(row)) continue;
    const seat = row.seat;
    const state = row.state;
    if (typeof seat !== "string" || typeof state !== "string" || seat === "" || state === "") continue;
    out.set(seat, state);
  }
  return out;
}

export function stateFor(s: SeatStates | undefined, cardId: string): string {
  if (!s || !cardId) return "";
  return s.get(`agent:${cardId}`) ?? "";
}

export function sameStates(a: SeatStates, b: SeatStates): boolean {
  if (a.size !== b.size) return false;
  for (const [k, v] of a) {
    if (b.get(k) !== v) return false;
  }
  return true;
}

export function startSeatPoll(o: {
  invoke: (cmd: string) => Promise<unknown>;
  hasSeats: () => boolean;
  onChange: (s: SeatStates) => void;
  everyMs?: number;
}): () => void {
  const asked = o.everyMs ?? FLOOR_MS;
  const every = Number.isFinite(asked) ? Math.max(FLOOR_MS, asked) : FLOOR_MS;
  let stopped = false;
  let inflight = false;
  let last: SeatStates = new Map();
  const tick = () => {
    if (stopped || inflight || !o.hasSeats()) return;
    inflight = true;
    let pending: Promise<unknown>;
    try {
      pending = Promise.resolve(o.invoke("seat_outcomes"));
    } catch (err) {
      pending = Promise.reject(err);
    }
    pending
      .then((v) => {
        if (stopped) return;
        const next = parseDoor(v);
        if (sameStates(last, next)) return;
        last = new Map(next);
        o.onChange(last);
      })
      .catch(() => {})
      .finally(() => {
        inflight = false;
      });
  };
  const timer = setInterval(tick, every);
  return () => {
    stopped = true;
    clearInterval(timer);
  };
}
