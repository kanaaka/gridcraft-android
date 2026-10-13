# Port notes — GridCraft on Android

Upstream: [`storytold/gridcraft`](https://github.com/storytold/gridcraft) @
`a5137872c15fac7bf260fa2d25ea8f7e66b19dc2` (v0.5.0)
Published APK: `gridcraft-0.5.0-arm64-v8a-v3.apk`

The port is `apps/gridcraft-android/` (`src/lib.rs` 462 lines, `src/file_picker.rs` 403,
`src/modifiers.rs` 202, `src/prefs_watch.rs` 134) plus **7 upstream files** — a save-dialog seam
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

Four unit tests sit beside it in `crates/engine/src/tests.rs` —
`auto_sum_total_follows_the_column_number_format`,
`auto_sum_total_stays_general_for_unformatted_numbers`,
`auto_sum_single_cell_uses_the_format_of_the_range_above` and, for the Format Painter,
`format_painter_picks_up_and_applies_to_a_target_range`. **Checked:** in the assembled tree,
`cargo test -p gridcraft-engine` passes **4/4** for these, and the whole engine lib suite passes
**207/207**.

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

## 2026-10-13 — Two-finger scrolling (and the wheel Android never sends)

**The complaint:** on the tablet a finger drag selected text instead of moving the page, and there was
no way to scroll at all.

**Why:** every app in this suite scrolls from `egui::Event::MouseWheel` — `WordCraft`'s `scroll.rs` is
explicit about the contract (pixel deltas move the page 1:1, `TouchPhase::End` on the last event is the
"fingers lifted" signal that starts the kinetic coast, wheel notches count as three lines). None of
that runs on Android, because winit's Android backend has no mouse and therefore emits no wheel
events at all (`src/platform_impl/android/mod.rs` sends six window events and a wheel is not one of
them). Touch arrives as `WindowEvent::Touch`, which egui-winit forwards as `egui::Event::Touch` *and*
turns into an emulated mouse pointer — so a finger drags a selection and the page never moves.

**The change — `src/touch_scroll.rs` (new, 231 lines, 3 tests), wired into the shell's
`raw_input_hook`:**

- one finger: untouched, so dragging still selects — the emulated pointer works as before;
- two or more fingers: the centroid's movement becomes `Event::MouseWheel` with
  `MouseWheelUnit::Point` and `TouchPhase::Move`;
- a final `TouchPhase::End` wheel event when the fingers lift, which is what makes the app coast to a
  stop instead of stopping dead;
- while a two-finger gesture is live the emulated **pointer button** events are dropped, or the first
  finger would drag a selection across the page at the same time as it scrolls. Pointer *position*
  events are kept, because the apps only scroll while the pointer is over the canvas;
- the touch events themselves stay in the batch: egui's pinch handling reads them, so a pinch still
  zooms and a two-finger *pan* moves nothing but the page.

**Ctrl+wheel zoom** works too, which is why `modifiers.rs`'s `apply()` now returns the state it tracked
(`egui::Modifiers`) instead of nothing: the bridge stamps it onto the wheel event it synthesises, and
the touch events carry it as well.

**Checked:** the three tests in `touch_scroll.rs` (one-finger pass-through, two-finger centroid →
wheel, the lift event) run on the host with `cargo test -p gridcraft-android`; the shell compiles for
`aarch64-linux-android`; and the string table of the `.so` inside the released APK carries the module.
On-device behaviour is the porter's tablet test, not an automated one.

## 2026-10-13 — `modifiers.rs` returns the tracked state

`apply(&mut self, raw)` now returns the `egui::Modifiers` it folded into the frame's events. The
synthesised two-finger wheel event has to carry modifiers itself (Ctrl+wheel is zoom in these apps,
plain wheel is scroll), and only the tracker knows them, so the state is handed back rather than
recomputed. Nothing else about the modifier handling changed.

## 2026-10-13 — The *Fonts* folder, in the shells that did not have one

PhotoCraft and VectorCraft already pointed their platform-font hook at a USB-visible
`Android/media/<package>/Fonts`; this shell did not, so there was no way to install a font on the
tablet at all.

The shell now creates `<App media>/Fonts`, sets `HOME` to the app's media directory, and symlinks
`.fonts` and `.local/share/fonts` to that one folder, so every path a font scan is likely to consult
lands in the same visible place. A `.ttf`/`.otf` dropped in over a cable appears in the app's font
list on the next scan. `Android/media` rather than `Android/data`, deliberately: Android 11+ hides
`Android/data` from MTP.

## 2026-10-13 — The picker no longer tells a *granted* user to grant access

The empty-folder message in shared storage used to end with "Grant *All files access*…", and it was
shown to everyone, including users who had already granted it. The trigger had been
`/storage/emulated/0/Android/data` failing to list — **measured on the emulator: that folder stays
unlistable to the app on API 35 even with the grant**, so the tell was wrong and the advice was
insulting to the one user who had done what it asked.

Now the note is conditional on the folder being shared storage, is worded as a possibility ("If your
files are here, the app may lack shared-storage access…"), and leaves the decision to the user,
because the grant cannot be probed from inside the app.

The USB tip became folder-dependent for the same reason — Android 11+ hides `Android/data` from MTP,
so a file copied there over a cable never arrives and the folder merely looks empty (which reads as a
picker bug), while `Android/media` *is* reachable over a cable. Saying the wrong one of those two is
worse than saying nothing.
## 2026-10-13 — Ctrl shortcuts: the tracker reads the physical key too

### Fixed
- **Ctrl+anything did nothing, while Shift+click and Shift+arrow worked.** winit's Android
  logical-key table names `Shift` and `Alt` but **not** `Control` — so a Ctrl press arrives with an
  unidentified *logical* key (egui-winit falls back to the physical one) while its physical key is
  correctly `ControlLeft`/`ControlRight`. `src/modifiers.rs` matched the logical key alone, so it
  never saw Ctrl go down: Ctrl+A, Ctrl+C, Ctrl+V, Ctrl+S, Ctrl+B and every other modifier shortcut
  was dead, and only the modifiers winit happens to name worked. The tracker now reads the physical
  key as well as the logical one, on both left and right, and keeps everything else: the per-event
  modifier copies (so a press and a click in one frame stay ordered) and `ModifiersChanged` as the
  authority (so a modifier held while the app goes to the background does not stick).
  The same tracker is in all four ports of this set; it is verified end-to-end in WordCraft
  (`Ctrl+A` selects the text, `Ctrl+B` emboldens it). This app's own shortcuts are the tablet check.

## What is deliberately *not* in the patch

`Cargo.lock` is excluded on purpose: it moves only because the port crate is a new workspace member,
and `cargo` regenerates it on the first build. The patch contains only changes that matter to a reader
— every tracked file the port touches, plus any new file that has to live inside an upstream directory.

## Not verified here

The APK attached to **v0.1.2** was built from exactly this tree on 2026-10-13 (upstream pin + the patch + the port crate), and the checks in the README's verification table were run against it. What that does *not* buy is a runtime claim: it was not launched on a device or emulator as part of publishing, so every behavioural statement below rests on the hand-testing the porter did on an arm64 tablet. The engine tests
*were* run here (`cargo test -p gridcraft-engine` in the assembled tree: 207 passed, 0 failed), but no
device test. The runtime fixes above came from the porter's hand-testing on an arm64 tablet; there
is no automated device test in this repository.
