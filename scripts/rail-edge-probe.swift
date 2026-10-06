import CoreGraphics
import Foundation

// Read-only. Lists windows that cross the primary left-edge band, then
// every Orbitty window. Does not move the cursor or post events.

let band = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]]
print("BAND on-screen windows intersecting x<12, y 79..1552 (front to back)")
var i = 0
for w in band {
  i += 1
  let owner = w[kCGWindowOwnerName as String] as? String ?? ""
  let b = w[kCGWindowBounds as String] as? [String: Any] ?? [:]
  let x = (b["X"] as? Double) ?? 0
  let y = (b["Y"] as? Double) ?? 0
  let ww = (b["Width"] as? Double) ?? 0
  let hh = (b["Height"] as? Double) ?? 0
  let layer = w[kCGWindowLayer as String] as? Int ?? -1
  let num = w[kCGWindowNumber as String] as? Int ?? 0
  let pid = w[kCGWindowOwnerPID as String] as? Int ?? 0
  if x < 12 && x + ww > 0 && y < 1552 && y + hh > 79 {
    print(i, owner, "pid", pid, "win", num, "layer", layer, "x", x, "y", y, "w", ww, "h", hh)
  }
}

print("ORBITTY windows (optionAll)")
let all = CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID) as! [[String: Any]]
for w in all {
  let owner = w[kCGWindowOwnerName as String] as? String ?? ""
  if !owner.lowercased().contains("orbitty") { continue }
  let b = w[kCGWindowBounds as String] as? [String: Any] ?? [:]
  let layer = w[kCGWindowLayer as String] as? Int ?? -1
  let onscreen = w[kCGWindowIsOnscreen as String] as? Bool ?? false
  let alpha = w[kCGWindowAlpha as String] as? Double ?? -1
  let num = w[kCGWindowNumber as String] as? Int ?? 0
  let pid = w[kCGWindowOwnerPID as String] as? Int ?? 0
  print(owner, "pid", pid, "win", num, "layer", layer, "on", onscreen, "alpha", alpha,
        "x", b["X"] ?? "?", "y", b["Y"] ?? "?", "w", b["Width"] ?? "?", "h", b["Height"] ?? "?")
}
