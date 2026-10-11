/** Tile veil. The opacity slider scales this. It does not paint the page root. */
export const GLASS_VEIL_ALPHA = 0.72;

export function veilAlpha(clampedSlider: number): number {
  return clampedSlider * GLASS_VEIL_ALPHA;
}

/** A light slab is every channel at least 230 with alpha at least 0.5. */
export function isLightSlab(r: number, g: number, b: number, a: number): boolean {
  return a >= 0.5 && r >= 230 && g >= 230 && b >= 230;
}

export const REDUCED_TRANSPARENCY_QUERY = "(prefers-reduced-transparency: reduce)";

/** Reduced transparency skips the native vibrancy view. */
export function vibrancyWanted(prefersReducedTransparency: boolean): boolean {
  return !prefersReducedTransparency;
}

/** Solid tiles use the panel's own alpha. Otherwise the slider scales the veil. */
export function tileFillAlpha(clampedSlider: number, solid: boolean): number {
  if (solid) return 1;
  return veilAlpha(clampedSlider);
}
