/** Tile veil. The opacity slider scales this. It does not paint the page root. */
export const GLASS_VEIL_ALPHA = 0.72;

export function veilAlpha(clampedSlider: number): number {
  return clampedSlider * GLASS_VEIL_ALPHA;
}

/** A light slab is every channel at least 230 with alpha at least 0.5. */
export function isLightSlab(r: number, g: number, b: number, a: number): boolean {
  return a >= 0.5 && r >= 230 && g >= 230 && b >= 230;
}
