/** Tile veil. The opacity slider scales this. It does not paint the page root.
 *  Reduced transparency stays on this alpha. The non-RT card tint is separate. */
export const GLASS_VEIL_ALPHA = 0.72;

/** Dark-theme card tint over the per-card blur. 35–55%. Not the RT veil. */
export const GLASS_CARD_TINT_ALPHA = 0.45;

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

/**
 * Tile veil. Reduced transparency stays on this same translucent alpha.
 * It does not paint an opaque fill. `solid` is kept so the caller still
 * records the media query; the alpha does not become 1.
 */
export function tileFillAlpha(clampedSlider: number, solid: boolean): number {
  if (solid) return Math.min(0.85, veilAlpha(clampedSlider));
  return veilAlpha(clampedSlider);
}
