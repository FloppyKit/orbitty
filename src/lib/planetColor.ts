// GO-ORBITTY-PLANET-MARK: TS twin of habitat_mac/mod.rs `color_for` —
// byte-sum × golden angle, HSV s=0.55 v=0.80. Lists wear the same marble
// color as the disc. One hash, two languages; keep them in sync and never
// persist a color field.

export function planetRgb(id: string): [number, number, number] {
  // Rust sums UTF-8 bytes; TextEncoder matches for non-ASCII ids too.
  const bytes = new TextEncoder().encode(id);
  let sum = 0;
  for (const b of bytes) sum += b;
  const hue = ((sum * 137.508) % 360) / 360;
  return hsv(hue, 0.55, 0.8);
}

export function planetCss(id: string): string {
  const [r, g, b] = planetRgb(id);
  return `rgb(${Math.round(r * 255)}, ${Math.round(g * 255)}, ${Math.round(b * 255)})`;
}

function hsv(h: number, s: number, v: number): [number, number, number] {
  const i = Math.floor(h * 6);
  const f = h * 6 - i;
  const p = v * (1 - s);
  const q = v * (1 - f * s);
  const t = v * (1 - (1 - f) * s);
  switch (((i % 6) + 6) % 6) {
    case 0:
      return [v, t, p];
    case 1:
      return [q, v, p];
    case 2:
      return [p, v, t];
    case 3:
      return [p, q, v];
    case 4:
      return [t, p, v];
    default:
      return [v, p, q];
  }
}
