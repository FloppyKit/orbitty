/** Pure coach-line decisions. No Svelte. node:test imports this file. */

export const COACH_LINE = "Claim a window to make a walker.";
export const COACH_CLAIM_LABEL = "Claim this window";
export const COACH_TIP =
  "Claims the window you used last and puts its walker on a planet. No planets yet? You'll name one.";

export const COACH_CLAIM_STOP = "rcoach-claim";
export const COACH_SKIP_STOP = "rcoach-skip";

export interface CoachStory {
  v: number;
  skipped: boolean;
  coachDone: boolean;
  seededProjectId?: string | null;
  commsBuilt?: boolean;
  starsBuilt?: boolean;
  introDone?: boolean;
}

export function normalizeStory(raw: Partial<CoachStory> | null | undefined): CoachStory {
  return {
    v: raw?.v ?? 0,
    skipped: !!raw?.skipped,
    coachDone: !!raw?.coachDone,
    seededProjectId: raw?.seededProjectId ?? null,
    commsBuilt: !!raw?.commsBuilt,
    starsBuilt: !!raw?.starsBuilt,
    introDone: !!raw?.introDone,
  };
}

export function coachDue(input: {
  trusted: boolean;
  stateLoaded: boolean;
  claimsCount: number;
  story: CoachStory;
  embedded: boolean;
}): boolean {
  return (
    input.trusted &&
    input.stateLoaded &&
    input.claimsCount === 0 &&
    input.story.v === 1 &&
    !input.story.coachDone &&
    !input.story.skipped &&
    !input.embedded
  );
}

/** Armed fresh desk with nothing on it still shows the wheel (+ Project). */
export function emptyDeskShowsWheel(input: {
  windows: number;
  projects: number;
  story: CoachStory;
}): boolean {
  if (input.windows !== 0 || input.projects !== 0) return false;
  return input.story.v === 1 && !input.story.skipped;
}

export function coachStops(due: boolean): { id: string; group: "wheel" }[] {
  if (!due) return [];
  return [
    { id: COACH_CLAIM_STOP, group: "wheel" },
    { id: COACH_SKIP_STOP, group: "wheel" },
  ];
}
