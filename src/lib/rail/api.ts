import type { WindowInfo } from "../persist";

/** Inspector habitat members on the rail brain. */
export interface RailHabitatApi {
  railBoardCols: string[];
  toggleBoardCol: (col: string) => void;
  raisePlanetWindows: (pid: string) => Promise<void>;
  openPlanetTodos: (pid: string) => Promise<void>;
  focusRow: (w: WindowInfo) => Promise<void>;
  planetCount: (pid: string) => number;
}

/** RailPage brain surface for DIET-RAIL-SPLIT modules.
 *  Habitat members are required; the rest of the brain stays open. */
export type RailApi = RailHabitatApi & Record<string, any>;
