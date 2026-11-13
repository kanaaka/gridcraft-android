# Changelog

## 0.5.0-files-ru.3 — 2026-11-13

- Возвращён исходный встроенный выбор файлов для открытия и вставки рисунков.
  Системный выбор документов не вызывается.
- Фильтр содержит все 14 расширений движка, включая XLTX/XLTM, XLT, TAB и SCJSON;
  регистр и русские имена файлов не влияют на отображение. Начальная папка выбирается
  по наличию совместимых документов, переходы запоминаются до закрытия окна приложения.
- Закрытые общие папки больше не выглядят доступными только из-за успешного read_dir.
  Java проверяет фактический доступ Android, включая legacy storage на Android 10.
- «Открыть с помощью» принимает content/file URI с любым MIME и без MIME, VIEW/EDIT,
  а также SEND/SEND_MULTIPLE через EXTRA_STREAM и ClipData.
- singleTask предотвращает создание второго NativeActivity из штатного менеджера
  HONOR и выводит существующий редактор с открытым файлом на передний план.
- Завершён перевод названий команд, вложенных меню, сводных таблиц, палитр,
  параметров диаграмм, подсказок и статистики выделения; поиск команд работает по-русски.
- 13 тестов оболочки и 65 тестов интерфейса проходят. Добавлены проверки всех
  расширений, начальной папки и русских подписей каждой команды движка.

APK: `gridcraft-0.5.0-arm64-v8a-files-ru3.apk`, versionCode `16778502`.
Ключ подписи совпадает с files-ru.2.
Проверено на HONOR MLA-W09 с Android 16: встроенное открытие, X-plore, штатные «Файлы»,
холодный и повторный запуск, SEND; 16/16 проверок Android resolver проходят.
Проверки на планшете: [docs/BUILTIN-PICKER-RU3.md](docs/BUILTIN-PICKER-RU3.md).

## 0.5.0-files-ru.1 — 2026-11-13 (file access, Open with, Russian picker)

| | |
|---|---|
| APK | `gridcraft-0.5.0-arm64-v8a-files-ru.apk` (11,625,340 bytes) |
| sha256 | `042fcc9329d96f79a97405fff041d1265e82bbb85770c5472fb9b0750538248c` |
| Built from | upstream `storytold/gridcraft` @ `a5137872c15fac7bf260fa2d25ea8f7e66b19dc2` + this repository at the commit that released it |
| Package | `ai.storyteller.gridcraft` — versionName `0.5.0-files-ru.1`, versionCode `16778500`, minSdk 26, targetSdk 35 |
| ABI | `arm64-v8a` only |
| Signature | **new** throwaway local test key (`gridcraft-sideload.keystore`) — uninstall any earlier local APK first |

### Fixed
- **The picker could not see shared storage and never asked for it.** "All files access"
  (`MANAGE_EXTERNAL_STORAGE`) is declared but is not a runtime permission: the app has to open the
  system screen itself. The new `src/permissions.rs` detects the grant (`isExternalStorageManager()`
  on Android 11+, `checkSelfPermission` before that) and the Java Activity opens the right screen on
  the UI thread. The picker asks once per process, keeps an **Allow file access** button, makes a
  padlocked folder tappable, starts Open in `Documents`/`Download` once readable, follows the
  device's actual storage root and lists removable volumes. Startup logs whether access is granted.
- **Open lost the workbook's path.** Opening through the picker now runs `file.open` with the
  original path — a new upstream `pick_open_async` seam, the sibling of `pick_save_async` — so the
  title bar, Recent and Ctrl+S know which file is open.
- **GridCraft was absent from *Open with*.** The Activity advertises ACTION_VIEW +
  CATEGORY_DEFAULT for xlsx/xltx, xlsm/xltm, xlsb, xls, ods, csv/tsv, txt and json plus
  `application/octet-stream`; a file manager's file opens on a cold start or in the running window
  (`singleTop`). Content URIs are copied on a worker before their temporary grant expires; a
  readable `file:` URI is opened in place. An incoming workbook opens in its own tab, so unsaved
  work is not closed.
- **The picker was English only.** Its labels and the dialog titles now follow the editor's
  interface language; Russian is complete, including the permission banner and failure notices.
  Save/Open also gained **Show all files**.
- **`cargo apk build` failed with current cargo-apk.** It derives the packed library and
  `android.app.lib_name` from the package name, so `[lib] name` is now `gridcraft_android`; the
  packaged library is `lib/arm64-v8a/libgridcraft_android.so`.
- **The keyboard was drawn over the grid.** Both themes carry `windowSoftInputMode=adjustResize`,
  which matters more in a spreadsheet than in a text document: the cell being edited sits under
  the keyboard.

### Packaging
`scripts/build-apk.ps1` now completes the build: cargo-apk's native APK plus `javac`/D8 for
`GridcraftActivity`, Android 10 legacy-storage opt-out, `hasCode`, version bump, resource rebuild
with `aapt`, ZIP alignment and signing. `scripts/package-bridge.py` does the manifest/DEX merge.
All 11 shell tests pass; the patch applies to a clean checkout and the assembled tree matches the
build workspace. Not run on a physical device from this machine.

## v0.1.2 — 2026-10-13 (Ctrl shortcuts)

| | |
|---|---|
| APK | `gridcraft-0.5.0-arm64-v8a-20261013b.apk` (11.1 MB) — supersedes the v0.1.1 asset, same signing key |
| sha256 | `930bec5b49369e12579d2b4e2f299f71df07fb28a7ce08932f1fc8d7005b645f` |
| Built from | upstream `storytold/gridcraft` @ `a5137872c15fac7bf260fa2d25ea8f7e66b19dc2` + this repository at the commit that released it |
| Package | `ai.storyteller.gridcraft` — versionName `0.5.0`, versionCode `16778496`, minSdk 26, targetSdk 35 |
| ABI | `arm64-v8a` only |

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
