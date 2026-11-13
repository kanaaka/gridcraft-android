# Built-in file access and Open with — 2026-11-13

Open, Save As, Insert Picture and the Backstage exports use the built-in egui file picker; it is
not replaced by Android's system document chooser. The picker is translated into Russian using the
editor's language, checks the actual storage grant (a successful `read_dir` can still hide shared
documents), refreshes grants/revocations, and requests permission through the Java Activity's UI
thread. After granting access it navigates to shared `Documents`/`Download`. Storage roots follow
the actual Android user/profile, and removable volumes are listed separately. A **Show all files**
checkbox lets users inspect files hidden by the extension filter. Opening a workbook through this
picker passes its original path to the engine: the title bar, Recent and `Ctrl+S` all see it.

The manifest advertises ACTION_VIEW plus CATEGORY_DEFAULT for XLSX/XLTX, XLSM/XLTM, XLSB, XLS, ODS,
CSV/TSV, plain text and JSON, plus `application/octet-stream`. MIME-only filters match `content`
and `file` URI schemes without claiming web links; both the official and lower-case spellings of
the `macroEnabled` types are present because Android's resolver is case-sensitive and file
managers differ. The Java `GridcraftActivity` handles `onCreate` and `onNewIntent` (`singleTop`);
it opens readable `file:` URIs where they lie and imports `content:` URIs through
`ContentResolver` while the incoming temporary read grant is valid. Imports run on an ordered
worker and preserve filenames/extensions, including Unicode. Metadata failures fall back to the
URI name/MIME type. Each Activity owns its own queue of imported paths.

Rust polls that Activity's queue once idle and opens the document through `SheetApp::open_path`,
which is the engine's normal `file.open` command: warnings and Recent are handled as for any other
open, and because GridCraft is multi-document the incoming workbook appears in its own tab — an
unsaved workbook is never closed behind the user's back. A `content:` URI produces a private copy;
use Save As to choose a shared destination. Files opened through the built-in picker remain the
original filesystem documents.

The Windows packaging script includes the Java DEX, the Android 10 legacy-storage opt-out,
alignment and signing. VersionName is `0.5.0-files-ru.1`, versionCode `16778500`. It also corrects
the library target: cargo-apk 0.9.7 derives the packaged `.so` name and `android.app.lib_name` from
the *package* name, so the crate builds `libgridcraft_android.so`. The output is
`gridcraft-0.5.0-arm64-v8a-files-ru.apk`.

All 11 host tests passed, including Russian Open and Save dialog rendering, Unicode/uppercase
filenames, the device's actual storage root, opening an external workbook without losing unsaved
work, and the asynchronous Open answer arriving as a path. This is source-side verification only;
the APK has not been run on a physical device from this machine.

Final artifact verification: versionCode 16778500, arm64-v8a, Android 8+; signatures v2/v3
verified, signing certificate is the new local test key. ACTION_VIEW, CATEGORY_DEFAULT and the
required spreadsheet MIME types are present in the packaged manifest. Both `onCreate` and
`onNewIntent`, `takeOpenedDocument` and `requestStorageAccess` are present in `classes.dex`, and
`hasCode`/`requestLegacyExternalStorage` are true. The packaged native library is
`lib/arm64-v8a/libgridcraft_android.so`, its `android.app.lib_name` matches, and the
`adjustResize` theme item survived resource compilation.

APK: gridcraft-0.5.0-arm64-v8a-files-ru.apk (11,625,340 bytes)
SHA-256: 042fcc9329d96f79a97405fff041d1265e82bbb85770c5472fb9b0750538248c
