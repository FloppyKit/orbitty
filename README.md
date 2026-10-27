# Orbitty

A Mac app for the work that is actually on your screen. You claim a window onto a project. A small walker stands there until you are done.

This copy is for friends trying the alpha. It is signed on a Mac, not by Apple, and it is not notarized. The only promise is that the whole program is local files on your Mac: the app in `/Applications`, and your board in `~/Library/Application Support/space.orbitty.alpha`. A model you connect gets what you send it.

Apple silicon Macs (M1 or newer) only. If `BUILD_INFO.txt` says `architectures: x86_64 arm64`, that disk image is universal and also runs on Intel Macs.

Site: [orbitty.space](https://orbitty.space)

## Install

1. Open the disk image. Read `Open Anyway - read me first` (the same steps are in `docs/TESTER_OPEN_ANYWAY.md` if you have the source).
2. Drag **Orbitty** onto **Applications**. Copy `BUILD_INFO.txt` before you eject. Do not run the app from the disk image.
3. Double-click Orbitty in Applications. macOS says it cannot verify the developer. Click **Done**. Do not click Move to Trash.
4. On macOS 15 or later, right-click then Open no longer gets past that block. Open System Settings → Privacy & Security, and click **Open Anyway**. Once per download.
5. If Open Anyway does not appear, the guide has one Terminal command that clears the download flag on `/Applications/Orbitty.app`.
6. Orbitty asks for Accessibility so it can list windows and see which one is focused (titles and metadata only). Turn Orbitty on, go back, and click **Retry**.

## The first ten minutes

You start with an empty board.

1. After Accessibility, the rail shows a short coach line and a **+ Project** tile, even if no windows are open. The orbitty.space link is on that rail.
2. Claim a window onto a project. Four starter todos appear on that planet. The first one points at Comms.
3. Click the Comms plot. The walker walks over and the Comms icon appears.
4. The companion stays hidden until that Comms outpost is built. It is a spirit only (no face). It introduces itself once, then you can open Stars the same way.
5. Press ⌘⇧O, or hover the screen edge, to raise the wheel.

## Models

The alpha starts at **No model**. Nothing of anyone else's setup is in this copy.

**Set up a model** looks for Ollama or LM Studio on your Mac. If neither is running, it links the Ollama download. Any local, API-key, or signed-in model works once you attach it.

Keys live in your Mac's Keychain, under names that start with `space.orbitty.alpha.`. They are not written into the app folder. Never paste a key into the chat. If you do, make a new one: the app will not send that message on, and it will tell you to roll the key.

## What stays on this Mac

The whole program is local files on your Mac.

- App: `/Applications/Orbitty.app`
- Board and other app files: `~/Library/Application Support/space.orbitty.alpha`

A model you connect gets what you send it. That is the model's own path, not a promise about this app.

## Uninstall

1. Quit Orbitty.
2. Drag Orbitty from Applications to the Trash.
3. Remove `~/Library/Application Support/space.orbitty.alpha`.
4. In Keychain Access, delete items whose name starts with `space.orbitty.alpha.`.

## Known issues

- The first open of each download is blocked until you use Open Anyway. Right-click Open does not skip that on macOS 15 or later.
- Accessibility is a separate grant for this copy (`space.orbitty.alpha`).
- The companion is spirit-only, and it stays hidden until you build the Comms outpost.
- Apple silicon Macs (M1 or newer) only, unless `BUILD_INFO.txt` says `architectures: x86_64 arm64`.

## Build from source

You do not need this to try the app. The source is at <https://github.com/FloppyKit/orbitty> (Apache-2.0). Friends should use the disk image.
