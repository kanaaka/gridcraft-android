# Changelog

## v0.1.1 — 2026-10-13 (scrolling, modifiers, fonts, picker wording)

| | |
|---|---|
| APK | `gridcraft-0.5.0-arm64-v8a-20261013.apk` (11.1 MB) — supersedes the v0.1.0 asset, same signing key |
| sha256 | `a2012e67571d098a2c39834f6683040b0269cb4da516d3fa7dbd163be92683ae` |
| Built from | upstream `storytold/gridcraft` @ `a5137872c15fac7bf260fa2d25ea8f7e66b19dc2` + this repository at the commit that released it |
| Package | `ai.storyteller.gridcraft` — versionName `0.5.0`, versionCode `16778496`, minSdk 26, targetSdk 35 |
| ABI | `arm64-v8a` only |

### Changed
- **Two-finger scrolling.** A two-finger drag now scrolls the sheet (synthesised `MouseWheel` events
  from the touch centroid, with the lift event that starts the kinetic coast); one finger still selects
  and a pinch still zooms. Ctrl+wheel zooms.
- **A *Fonts* folder** at `Android/media/<package>/Fonts` (with `.fonts` and `.local/share/fonts`
  symlinked to it), so a font copied in over a USB cable is picked up.
- **Picker wording corrected.** An empty folder in shared storage no longer tells a user who has
  already granted *All files access* to grant it, and the USB tip now matches the folder
  (`Android/data` is unreachable over a cable, `Android/media` is not).

### Notes
The build was made from exactly the source in this repository (upstream pin + the patch + the port
crate) and signed with the same local test key as v0.1.0, so it installs over it.


## v0.1.0 — first published port (GridCraft 0.5.0)

| | |
|---|---|
| APK | `gridcraft-0.5.0-arm64-v8a-v3.apk` (11.0 MB) |
| sha256 | `f9439c0c7e47db3ecf240e7ceee016f3bcad711051d29ab7fb4a670231b376e7` |
| Built from | upstream `storytold/gridcraft` @ `a5137872c15fac7bf260fa2d25ea8f7e66b19dc2` + `patches/0001-gridcraft-android-port.patch` + `apps/gridcraft-android/` |
| Package | `ai.storyteller.gridcraft` — versionName `0.5.0`, minSdk 26, targetSdk 35 |
| ABI | `arm64-v8a` only |
| Signature | throwaway local test key, not an official key |

### In this release
- Android shell crate: NativeActivity entry point, `Services` implementation, file logger, preferences
  written while the app runs (atomic), modifier-key synthesis, Activity not recreated on configuration
  change.
- **Save dialogs that do not hold the frame** (`pick_save_async` + `save_dialogs`): a saved workbook
  learns its path, so the title bar follows, `Ctrl+S` stops asking, closing stops prompting, and the
  file enters the recent list.
- **Recent documents updated on save** — Save As no longer leaves the file out of the Start screen.
- **Format Painter works**: click paints a cell, drag paints the dragged range.
- **AutoSum totals take the number format of the data** they sum (currency column → currency total),
  with four tests added beside it.
- In-app picker for Open and Save As, with cached listings, "Show all files", hidden-file count and
  self-explaining locked roots.
- Launcher icon, storage permissions, edge-to-edge theme variant.


### Known issues
See *Known gaps* in the [README](README.md) and [`docs/PORT-NOTES.md`](docs/PORT-NOTES.md).
