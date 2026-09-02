"""Crop + upscale regions of a render for close inspection (no deps)."""
import sys
from pathlib import Path

import bpy

SRC = Path(sys.argv[sys.argv.index("--") + 1])
OUT = Path(sys.argv[sys.argv.index("--") + 2])
# boxes in 256x384 render coords: (x0, y0, x1, y1), y from TOP
BOXES = {
    "eyes": (40, 110, 216, 190),
    "mouth": (78, 185, 178, 245),
    "browband": (40, 80, 216, 140),
}

img = bpy.data.images.load(str(SRC))
w, h = img.size
px = list(img.pixels)

for name, (x0, y0t, x1, y1t) in BOXES.items():
    y0, y1 = h - y1t, h - y0t  # blender rows are bottom-up
    cw, ch = x1 - x0, y1 - y0
    scale = 3
    out = bpy.data.images.new(name, width=cw * scale, height=ch * scale, alpha=True)
    opx = [0.0] * (cw * scale * ch * scale * 4)
    for y in range(ch * scale):
        for x in range(cw * scale):
            sx, sy = x0 + x // scale, y0 + y // scale
            si = (sy * w + sx) * 4
            di = (y * cw * scale + x) * 4
            opx[di:di + 4] = px[si:si + 4]
    out.pixels = opx
    out.filepath_raw = str(OUT / f"debug_{name}.png")
    out.file_format = "PNG"
    out.save()
    print("CROP", name, out.filepath_raw)
