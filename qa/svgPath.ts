/** Sample SVG path data. Does not import stage geometry. */

export function svgPathPoints(d: string, perArc = 256): { x: number; y: number }[] {
  const pts: { x: number; y: number }[] = [];
  let i = 0;
  let cx = 0;
  let cy = 0;
  let sx = 0;
  let sy = 0;
  let cmd = "";
  const len = d.length;

  function skip() {
    while (i < len && (d[i] === " " || d[i] === "," || d[i] === "\n" || d[i] === "\t" || d[i] === "\r")) i++;
  }

  function readNum(): number | null {
    skip();
    if (i >= len) return null;
    const m = /^[+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?/.exec(d.slice(i));
    if (!m) return null;
    i += m[0].length;
    return Number(m[0]);
  }

  function push(x: number, y: number) {
    const last = pts[pts.length - 1];
    if (last && Math.hypot(last.x - x, last.y - y) < 1e-6) return;
    pts.push({ x, y });
  }

  while (i < len) {
    skip();
    if (i >= len) break;
    const c = d[i];
    if (/[MLAmlazZ]/.test(c)) {
      cmd = c;
      i++;
    } else if (!cmd) {
      break;
    }
    if (cmd === "M" || cmd === "m") {
      const x = readNum();
      const y = readNum();
      if (x == null || y == null) break;
      cx = cmd === "m" ? cx + x : x;
      cy = cmd === "m" ? cy + y : y;
      sx = cx;
      sy = cy;
      push(cx, cy);
      cmd = cmd === "m" ? "l" : "L";
      continue;
    }
    if (cmd === "L" || cmd === "l") {
      const x = readNum();
      const y = readNum();
      if (x == null || y == null) break;
      cx = cmd === "l" ? cx + x : x;
      cy = cmd === "l" ? cy + y : y;
      push(cx, cy);
      continue;
    }
    if (cmd === "A" || cmd === "a") {
      const rx = readNum();
      const ry = readNum();
      const phi = readNum();
      const fa = readNum();
      const fs = readNum();
      const x = readNum();
      const y = readNum();
      if (rx == null || ry == null || phi == null || fa == null || fs == null || x == null || y == null) break;
      const x2 = cmd === "a" ? cx + x : x;
      const y2 = cmd === "a" ? cy + y : y;
      for (const p of sampleArc(cx, cy, rx, ry, phi, fa, fs, x2, y2, perArc)) push(p.x, p.y);
      cx = x2;
      cy = y2;
      continue;
    }
    if (cmd === "Z" || cmd === "z") {
      cx = sx;
      cy = sy;
      push(cx, cy);
      cmd = "";
      continue;
    }
    break;
  }
  return pts;
}

/** SVG 1.1 F.6.5 endpoint-to-centre conversion, then uniform samples along the arc. */
function sampleArc(
  x1: number,
  y1: number,
  rx: number,
  ry: number,
  phiDeg: number,
  fa: number,
  fs: number,
  x2: number,
  y2: number,
  perArc: number,
): { x: number; y: number }[] {
  if (Math.hypot(x2 - x1, y2 - y1) < 1e-9) return [{ x: x2, y: y2 }];
  rx = Math.abs(rx);
  ry = Math.abs(ry);
  if (rx < 1e-9 || ry < 1e-9) return [{ x: x2, y: y2 }];
  const phi = (phiDeg * Math.PI) / 180;
  const cos = Math.cos(phi);
  const sin = Math.sin(phi);
  const dx = (x1 - x2) / 2;
  const dy = (y1 - y2) / 2;
  const x1p = cos * dx + sin * dy;
  const y1p = -sin * dx + cos * dy;
  const lam = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
  if (lam > 1) {
    const scale = Math.sqrt(lam);
    rx *= scale;
    ry *= scale;
  }
  const num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p;
  const den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
  let coef = Math.sqrt(Math.max(0, num / den));
  if (fa === fs) coef = -coef;
  const cxp = coef * ((rx * y1p) / ry);
  const cyp = coef * (-(ry * x1p) / rx);
  const cx = cos * cxp - sin * cyp + (x1 + x2) / 2;
  const cy = sin * cxp + cos * cyp + (y1 + y2) / 2;
  const ang = (ux: number, uy: number, vx: number, vy: number) => Math.atan2(ux * vy - uy * vx, ux * vx + uy * vy);
  const t1 = ang(1, 0, (x1p - cxp) / rx, (y1p - cyp) / ry);
  let dt = ang((x1p - cxp) / rx, (y1p - cyp) / ry, (-x1p - cxp) / rx, (-y1p - cyp) / ry);
  if (!fs && dt > 0) dt -= 2 * Math.PI;
  else if (fs && dt < 0) dt += 2 * Math.PI;
  const steps = Math.max(perArc, Math.ceil((Math.max(rx, ry) * Math.abs(dt)) / 1));
  const out: { x: number; y: number }[] = [];
  for (let s = 0; s <= steps; s++) {
    const t = t1 + (dt * s) / steps;
    const ct = Math.cos(t);
    const st = Math.sin(t);
    out.push({
      x: cx + rx * ct * cos - ry * st * sin,
      y: cy + rx * ct * sin + ry * st * cos,
    });
  }
  return out;
}
