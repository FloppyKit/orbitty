# Orbitty

Orbitty is a Mac app for the windows you are actually working on. You claim a window onto a planet (a project), and that window becomes a small walker on the planet. Windows you have not claimed stay out of the way: the wheel may list them, and the desk stays empty.

Orbitty is not a medical record. It keeps titles and metadata only, never the contents of a window.

The app id is `space.orbitty.mac`.

## Alpha

This is a small private alpha. The copy you were given is signed on the machine that built it, under the name Orbitty Local Codesign. Apple has not notarized it, so the first open is blocked until you use Open Anyway below. It runs on Apple silicon. Expect rough edges.

There is no companion in this alpha. Send feedback to the person who invited you.

## Requirements

- An Apple silicon Mac
- macOS 15 or later (tested on macOS 26)
- An administrator password, once, for Open Anyway
- Accessibility permission

## Install

1. Open the DMG (the disk image).
2. Drag **Orbitty** onto **Applications**.
3. Eject the DMG.
4. Open Orbitty from Applications.

Do not open the app from the DMG. macOS would run a temporary copy, and the Accessibility permission would not stay with it.

## Open Anyway

Because the app is not notarized, macOS blocks the first launch from a download.

1. Double-click Orbitty in Applications. macOS says it cannot verify the developer. Click **Done**. Do not click Move to Trash.
2. On macOS 15 and later, right-click → Open no longer works (Control-click no longer works either).
3. Open **System Settings → Privacy & Security**. Scroll to Security. Next to the note that Orbitty was blocked, click **Open Anyway**. Enter your administrator password, then click **Open**.
4. Do this once per download. A new download asks again.

If **Open Anyway** never appears, quit Orbitty and run this in Terminal, then open the app from Applications again:

```bash
xattr -dr com.apple.quarantine /Applications/Orbitty.app
```

The same steps, written out on their own page, are in [docs/TESTER_OPEN_ANYWAY.md](docs/TESTER_OPEN_ANYWAY.md). You do not need that page. The steps above are enough.

## Accessibility

Orbitty needs Accessibility to list windows and to focus one. It reads the window title and the app name. It does not read what is on the page or in the document.

If the permission is missing, the wheel shows **Accessibility access required**, the path of this copy of the app, and **Retry**.

1. Open **System Settings → Privacy & Security → Accessibility**.
2. Turn on the toggle for this app.
3. Click **Retry**. The view updates on its own once the grant lands.

macOS ties that grant to this copy's path and signature. If you move the app, or the signature changes, turn the toggle on again for the path Orbitty shows you. The same signing identity should keep the grant when an update replaces the app in place. That has not been confirmed on every Mac. If a new copy asks again, toggle it on and tell the person who invited you.

## Your first minute

The wheel rail is the way in. It hides on an edge of the primary display (the left edge, unless you change it). Hover that edge, or press **⌘⇧O**, and it comes out. Press **⌘⇧O** again to put it away.

Open **Settings** on the wheel. The Rail group sets the edge (left, right, top, or bottom) and whether the wheel hides when you leave it. Hiding when focus leaves is on by default.

Planets are tiles on a half-wheel. Select a planet and the rest of the screen is a large preview of that planet.

1. Make a planet with **+ Project**. The name is the project.
2. Claim a window by dragging that window's row onto the planet.

On a fresh desk, once Accessibility is on and nothing is claimed yet, one line offers **Claim this window**. That claims the frontmost window that is not Orbitty. If you have no planet yet, it asks you to name one. You can dismiss the line. It does not come back.
3. The claimed window is a walker on that planet. Click the walker to bring the window back (focus it, or restore it if it was minimized).
4. When you are done with it, mark it Done. The walker walks off the planet.

That is the loop. A walker stays for an open window, a minimized window, or a window you have closed. A closed window keeps a dim walker on its planet until you mark it Done or remove the claim. The wheel can still list windows you have not claimed. Those rows are not walkers.

## Local-first and privacy

What this install saves stays on this Mac, in:

`~/Library/Application Support/space.orbitty.mac`

`state.json` in that folder holds planets, claims, the board, and settings. `control.sock` and `control.token` sit beside it so the command-line tool can talk to a running Orbitty. The socket is local to this Mac. It is not a network port.

The live window list is not a copy of your screen. For each window, Orbitty keeps the title, the app name, the bundle id, whether it is minimized, and sometimes the path of a document the app says is open, when that file exists. It does not read the file.

There is no account. Orbitty does not send telemetry. It does not call a hosted model on its own. Sprite pictures are drawn in the app or imported from a file on this Mac. They are not downloaded.

A network connection happens only if one of these is true:

- You open a web link you attached to a card. Orbitty hands that address to macOS.
- You turn the private relay on. It is off by default. It may talk to this machine at `127.0.0.1`, or to a Tailscale address you set in `100.64/10`. Other hosts are refused. The alpha DMG does not turn the relay on.
- An optional model lane is configured and then used. That lane is not part of this alpha. It is covered under Build from source.

The app bundle includes microphone and speech-recognition reasons. They belong to a Talk button. This alpha does not start Talk, so it does not ask for the microphone unless that button is used.

To wipe this install, quit Orbitty and delete the folder above. That removes saved state, the socket, and the token. It does not remove the Accessibility toggle.

## Known issues

- Gatekeeper checks every new download. Open Anyway is once per download.
- Accessibility can ask again if the app moves or the signature changes. An update that stays in Applications and keeps the same signing identity should keep the grant. If it does not, turn the toggle on again.
- The wheel is on the primary display only.
- While the wheel is open, its window covers that display. Clicks on the wheel stay in Orbitty. Clicks on the empty part of the screen are captured until click-through is in your build. Press **⌘⇧O** to put the wheel away, then click what you wanted.
- When click-through is included, clicks outside the wheel pass through to the window behind them. Until then, use **⌘⇧O** as above.
- `orbittyctl` is not on your PATH. Use the copy inside the app, below.
- Intel Macs are not supported.

## Build from source

You need Rust (`cargo` on your PATH) and Node.js. From a checkout of this repo:

```bash
npm install
```

Create a code-signing identity first. In Keychain Access, choose Certificate Assistant → Create a Certificate, and set the type to Code Signing. Name it `Orbitty Local Codesign`, or set `ORBITTY_SIGN_IDENTITY` to a codesigning identity you already have. Do not sign ad hoc. `ORBITTY_SIGN_IDENTITY=-` is refused. An ad-hoc signature changes on every rebuild, and the Accessibility grant breaks each time.

```bash
./scripts/install-stable.sh
```

That builds a release app, signs it with that identity, and installs it to `~/Applications/Orbitty.app`. Open that app, and grant Accessibility for that path.

`scripts/gate.sh` runs the local checks. `scripts/gate.sh --quick` only scans for stray machine paths.

A companion seat is not in this alpha. `./scripts/install-kitchen.sh` is optional and for development. With no flags it prints usage and writes nothing. `--cli` copies the command-line tools onto your PATH. `--relay` starts a local relay. The alpha does not ask you to run this script. An optional model lane that calls a host is not part of the DMG. Configuring one is the other time the app uses the network.

## Command line

For the DMG install:

```bash
/Applications/Orbitty.app/Contents/MacOS/orbittyctl status
/Applications/Orbitty.app/Contents/MacOS/orbittyctl windows list
```

Orbitty has to be running. `status` is app health and the Accessibility summary. `windows list` is the current window list. Both talk to that running app over `control.sock`. If Orbitty is not running, the tool exits and changes nothing. The token is `control.token` in the same folder, unless you pass `--token` or set `ORBITTY_CONTROL_TOKEN`.

A build from `install-stable.sh` uses the same program names under `~/Applications/Orbitty.app/Contents/MacOS/`.

`orbitty-mcp`, next to `orbittyctl`, is that same socket over standard input and output. It does not listen on a network port.

## More

The product rules are in [docs/THIN_SLICE_LAWS.md](docs/THIN_SLICE_LAWS.md).

How the pieces sit: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

License: [Apache-2.0](LICENSE).
