import type { KanbanCard, WindowInfo } from "../persist";
import type { ArchState } from "./stageArch";

export interface ClaimFlash {
  pid: string;
  title: string;
  until: number;
}

/** Inspector habitat members on the rail brain. */
export interface RailHabitatApi {
  railBoardCols: string[];
  toggleBoardCol: (col: string) => void;
  raisePlanetWindows: (pid: string) => Promise<void>;
  ensureExpand: (pid: string) => void;
  openPlanetTodos: (pid: string) => Promise<void>;
  focusRow: (w: WindowInfo) => Promise<void>;
  planetCount: (pid: string) => number;
}

/** View-only arch. Nothing here is written to disk. */
export interface RailStageArchApi {
  arch: ArchState;
  archDo: (next: ArchState) => void;
  stageEmbed: "settings" | null;
  closeStageEmbed: () => void;
  popOutSettings: () => void;
  popOutBoard: () => void;
  planetCards: (pid: string) => KanbanCard[];
  spinCenter: number;
  reduceMotion: boolean;
  claimFlash: ClaimFlash | null;
}

/** RailPage brain surface for DIET-RAIL-SPLIT modules.
 *  Habitat members are required; the rest of the brain stays open. */
export type RailApi = RailHabitatApi & RailStageArchApi & Record<string, any>;
