# Port notes — GridCraft on Android

Upstream: [`storytold/gridcraft`](https://github.com/storytold/gridcraft) @
`a5137872c15fac7bf260fa2d25ea8f7e66b19dc2` (v0.5.0)
Published APK: `gridcraft-0.5.0-arm64-v8a-v3.apk`

The port is `apps/gridcraft-android/` (`src/lib.rs` 462 lines, `src/file_picker.rs` 403,
`src/modifiers.rs` 92, `src/prefs_watch.rs` 134) plus **7 upstream files** — a save-dialog seam
(`save_dialogs.rs`, new) and two spreadsheet bugs that surfaced while using the port, one of which
(`AutoSum`) is not Android-specific at all.

## 1. The shell crate

`gridcraft_ui_egui::Services` is a struct of `Option<Box<dyn Fn…>>` fields deriving `Default`. Android
is the third host to fill it, after the desktop binary and the browser shell. Load-bearing in every
port of this set:

- **`[lib] name = "gridcraft"` + `crate-type = ["cdylib"]`** → `lib/arm64-v8a/libgridcraft.so`, and the
  injected `android.app.lib_name` is that artifact name with `-` → `_`. A mismatch installs and then
  dies on launch.
- **`options.android_app = Some(app)`** — `eframe::run_native` errors without it.
- **A file logger in the app's own data directory** (`logcat` needs adb; a sideloaded build has none).
- **Preferences under `internal_data_path()`**: `ui.json` (the recent-documents list) and `prefs.json`,
  written while the app runs (§3).

## 2. Save dialogs that do not hold the frame (`pick_save_async` + `save_dialogs.rs`)

`Services::pick_save` is the *blocking* shape: the host shows a panel and returns the chosen path in
the same call. That works on the desktop (`rfd`) and cannot work on a host that draws its own picker
between frames — the browser, and Android, where there is no native dialog to call at all.

Handing the host finished **bytes** instead (the web `download` path) looks equivalent and is not: the
app is never told where the file went. `DocState::path` stays `None`, so the workbook is still
"Book1" with unsaved changes — the title bar never changes, `Ctrl+S` asks for a location again, and
closing prompts to save what was just saved. That is the first thing a user notices.

So the upstream patch adds the asynchronous seam: `Services::pick_save_async(&str) ->
Receiver<Option<String>>` (the host returns a receiver, an empty answer means cancelled), and a new
`crates/ui-egui/src/save_dialogs.rs` holding the pending save and the work to run when the path
arrives on a later frame. **Every** save dialog in the app goes through `SheetApp::ask_save`, so the
path can never be dropped on the floor. `poll_save()` runs from the frame loop, so what asked executes
on the frame the answer lands.

Two callers had their own reason to change:

- **Save As** used to only accept a path from the blocking dialog, and otherwise fell through to the
  bytes path. It now asks asynchronously and, only when the host has neither dialog, downloads.
- **Export As** (and the Copy/export dialog in `dialogs.rs`) used to run the export inside the same
  call. Now it asks and exports on the answer frame, and reports "handed over" rather than "written" —
  the workbook is untouched either way, because an export is a copy and must not become the workbook's
  own file.

One related detection was wrong and is fixed: the "this is the web, so download" branch keyed off
`pick_save` being absent, which is *also* true of any host that has only the asynchronous dialog. It
now checks for both.

## 3. Preferences are written while the app runs

The desktop writes its preferences on the way out; Android gives no such moment, so a swipe from
Recents took every preference with it — including the recent-documents list, which is why documents
saved on the tablet kept disappearing from the Start screen. `prefs_watch` writes them from the frame
loop: change-detected, at most every two seconds, atomically (temp file + rename), one writer per file
because there are two files.

## 4. A saved workbook did not appear in Recent

`open_path` puts an opened file at the top of the recent list; **nothing did it for saving.** A
workbook saved with Save As stayed out of Recent, so the only way back to it was to find it by hand.
The patch inserts the active document's path at the top of the list (and truncates to 20) as soon as
it has one — but only when it is not already first, so the list stays a list of files rather than of
whatever is on screen.

## 5. The Format Painter was dead

The ribbon's Format Painter *picked up* the selection's format and nothing ever delivered it to a cell:
the engine's `edit.formatPainter` command with an `apply` target was never called from the grid. A
plain click now lands it on the cell the click hit (Excel paints on the click), and a drag lands it on
the dragged range — but not while editing, where a select-drag is point mode for a formula reference,
not a paint. Shift and Command clicks extend the selection above that branch and never reach it. The
engine still owns the tiling across a range and the clearing of the one-shot painter.

## 6. AutoSum ignored the column's number format

Excel's AutoSum writes its total with the number format of the cells it added up, so a currency column
sums to a currency total. The engine wrote a `General` total. `auto_sum_num_fmt` now finds the number
format of the range's last numeric cell — scanning up from the bottom-right corner — and
`set_auto_sum_formats` derives it onto the total's **own** style, keeping the font, fill and borders
AutoSum wrote there; only the number format follows the data. An unformatted range stays `General`
rather than having a format invented for it, and the single-cell case takes the format of the range
above.

Three unit tests were added beside it in `crates/engine/src/tests.rs`
(`auto_sum_total_follows_the_column_number_format`,
`auto_sum_total_stays_general_for_unformatted_numbers`,
`auto_sum_single_cell_uses_the_format_of_the_range_above`), plus one for the Format Painter
(`format_painter_picks_up_and_applies_to_a_target_range`). Run them with
`cargo test -p gridcraft-engine`.

**Note:** §5 and §6 are fixes to the spreadsheet engine, not to Android — they are in this repository
because they were found while using the port. They are candidates for upstream.

## 7. Modifier keys, Activity recreation, permissions, icon

- **`modifiers.rs`**: winit's Android backend emits only six window events and never
  `ModifiersChanged`, and `egui-winit` reads modifiers from that event and nowhere else — so
  `ctx.input(|i| i.modifiers)` is the all-false default for the whole run, and Ctrl+S, Ctrl+Z,
  Ctrl+Shift+… and Shift-click do nothing. The key events do arrive, so the shell tracks the modifiers
  in `raw_input_hook` and re-injects them (held state, stamped onto every `Key`/`PointerButton` event,
  one `ModifiersChanged` per frame), carrying state **between** frames because the press and the click
  usually land in different ones.
- **`config_changes`** in the APK metadata, so rotation, docking, split-screen resize, density and
  font-scale changes do not recreate the Activity — a recreated Activity destroys the native window
  while the render thread lives on, which is a blank surface.
- **Permissions**: `MANAGE_EXTERNAL_STORAGE` (granted by hand under Settings ▸ Special access; Google
  Play restricts it to file-manager-class apps, which does not apply to a sideload), `READ_MEDIA_IMAGES`
  for Android 13+, and the legacy storage permissions capped with `max_sdk_version`.
- **Theme and icon**: the same two-variant edge-to-edge theme as its siblings (`targetSdkVersion 35`
  puts the app under a transparent status bar on Android 15, and winit 0.30 has no safe-area API), and
  launcher icons under `res/mipmap-*`.

## Upstream files this port changes (7)

| File | Change | Why |
|---|---|---|
| `crates/ui-egui/src/save_dialogs.rs` | **new** — pending save + what to run when the path arrives | a host that draws its own picker cannot return a path in the same call (§2) |
| `crates/ui-egui/src/lib.rs` | `Services::pick_save_async`, `pending_save`, `ask_save`, `poll_save`; Save As asks asynchronously; recent list updated on save | §2, §4 |
| `crates/ui-egui/src/backstage.rs` | export asks and writes on the answer frame; the "web" detection fixed | §2 |
| `crates/ui-egui/src/dialogs.rs` | the Copy/export dialog asks asynchronously when no name was typed | §2 |
| `crates/ui-egui/src/grid.rs` | Format Painter applied on click and on drag | §5 |
| `crates/engine/src/cmd/formulas.rs` | AutoSum total takes the range's number format | §6 |
| `crates/engine/src/tests.rs` | 4 tests for §5 and §6 | evidence |

## Not verified here

The APK in Releases was not re-run on a device while publishing this repository, and the four new
engine tests were added rather than shown passing here — run `cargo test -p gridcraft-engine` in the
assembled tree. The runtime fixes above came from the porter's hand-testing on an arm64 tablet; there
is no automated device test in this repository.
