# Open Orbitty (read this first)

This app is signed on a Mac, not by Apple. On macOS 15 or later the first open is blocked. Tested on macOS 26.

1. You need an Apple silicon Mac running macOS 15 or later.

2. Open the disk image. Drag **Orbitty** onto **Applications**. Eject the disk image. Do not run Orbitty from the disk image. macOS would start a temporary copy, and the Accessibility permission would not stick.

3. In Applications, double-click **Orbitty**. macOS says it cannot verify the developer. Click **Done**. Do not click Move to Trash. Right-click then Open no longer gets past this block on macOS 15 or later.

4. Open System Settings → Privacy & Security and scroll to Security. Next to the note that Orbitty was blocked, click **Open Anyway**. Enter an admin password, then click **Open**. Once per download.

5. Orbitty then asks for Accessibility. It uses that to list windows and see which one is focused (titles and metadata only). Open System Settings → Privacy & Security → Accessibility, turn Orbitty on, go back to Orbitty, and click **Retry**.

6. Only if Open Anyway does not appear, paste this in Terminal, then open Orbitty from Applications:

```
xattr -dr com.apple.quarantine /Applications/Orbitty.app
```

7. Updates: replace Orbitty in Applications with the new copy. Expect Open Anyway again on each download. Accessibility should carry over, because the signing identity stays the same. That is not verified on every Mac. If it does not carry over, tell the person who sent you the build.

8. This alpha has no companion. The wheel rail lives on the screen edge (hover the edge, or press ⌘⇧O). To uninstall: quit, drag Orbitty to the Trash, and optionally remove `~/Library/Application Support/space.orbitty.mac`.

9. If something goes wrong, send the macOS version, a screenshot, and `BUILD_INFO.txt` from this disk image.
