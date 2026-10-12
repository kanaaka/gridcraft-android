# Changelog

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
