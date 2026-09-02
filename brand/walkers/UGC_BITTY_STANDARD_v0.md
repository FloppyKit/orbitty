# UGC + ship-lean cast standard v0

## Product decision (Matt 2026-08-26)
- **Ship only 2 heads:**  
  - **Head A** — human androgynous female-lean (blessed plate)  
  - **Head B** — fuzzy outline (v0) + **cat mouth**  
- Optional **chin wider** slap-on for Head A  
- Variety from **eyes (mostly crazy + some normal)**, **hair**, **hats** — not 50 faces  
- Do **not** spend 9 months authoring every combo

## Paper-doll stack (engine + packs)
```
body (shared)
  └ head_a | head_b
       └ chin? (optional mesh/decal)
       └ eyes (slot)
       └ hair | hat (slot, mutually exclusive or hat-over-hair)
       └ outfit
       └ outline post (DQ8) optional
```

## Ship quantity out the gate (lean)
| Slot | Count | Notes |
|------|-------|-------|
| Heads | **2** | A human, B fuzzy-catmouth |
| Chin | 0–1 | wider slap |
| Eyes | ~8–12 | majority crazy, few normal/android/dot |
| Hair | ~6–10 | including silver electric |
| Hats | ~6–10 | bandana, brim, beanie, seasonal later |
| Bodies/outfits | few shared | reuse |

Combos explode mathematically without unique art per agent.

## Community UGC (the real scale plan)

### Why
First-party only finishes a **starter kit**. Long tail = community (same as Roblox shirts, VRoid hubs, TIAR/TE mods culture).

### Standard pack format (upload contract)
```
my-bitty-pack/
  pack.json          # id, name, author, license, slots, preview
  preview.png        # 256–512 store thumb
  heads/             # optional — advanced
  eyes/*.png         # RGBA, fixed canvas e.g. 256×256 or 832×1248 aligned
  hair/*.png
  hats/*.png
  outfits/*.png
  LICENSE            # CC0 / CC-BY / proprietary-with-permission
```

**pack.json (min):**
```json
{
  "schema": "orbitty.bitty.pack/v0",
  "id": "author.packname",
  "name": "Neon Season Eyes",
  "author": "handle",
  "license": "CC-BY-4.0",
  "slots": {
    "eyes": [{ "id": "spiral_pink", "file": "eyes/spiral_pink.png" }],
    "hair": [],
    "hats": []
  },
  "canvas": { "w": 256, "h": 256, "anchor": "face_center" },
  "compatible_heads": ["head_a", "head_b", "any"]
}
```

### App behavior
1. **Built-in starter pack** (our 2 heads + eye/hair/hat set)  
2. **Import pack** (.zip) → validate schema + image size + no path zipslip  
3. **License warn** (CC0/CC-BY preferred; block nothing early except malware zip)  
4. **Forum / board link** in app: “Get Bitties” → community gallery (can start as Discourse/GitHub releases/itch — no need custom store day 1)

### Templates for makers
- PSD/Pixelorama template with head A/B silhouettes as onion layers  
- Empty eye/hair/hat slots with guides  
- Export script or “save as pack” checklist  

### Competition / seasons (“next Bitties season”)
**Fun if interest exists; don’t build tournament infra first.**

| Stage | What |
|-------|------|
| 0 users | No competition — ship starter + import |
| Some packs appear | Spotlight “Pack of the week” manually on forum |
| Real heat | Seasonal brief: theme (neon, winter, DQ toast), prize = in-app badge / Orbitty merch / featured carousel |
| Assume interest? | **No** — announce *optional* season when ≥N packs or active thread; until then “gallery” is enough |

**Competition cost control:** Google Form + forum thread + manual feature. Not a full UGC economy.

## First-party art budget (anti-9-month)
1. Lock head A + head B PNGs  
2. One eye sheet (crazy-heavy)  
3. One hair sheet + one hat sheet  
4. Template + pack.json schema published  
5. Stop designing faces

## Non-goals
- Marketplace payments v0  
- Moderation AI  
- 3D VRM requirement for Bitties  

## Mouth slot (2026-08-26)
Head B furry chassis supports **mouth variants**: cat, dog (lizard later via UGC).
First-party lizard/dino head = **not** v0 core — see CAST_HEAD3_LIZARD_v0.md.
