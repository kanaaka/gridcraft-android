# GridCraft for Android (arm64-v8a)

## Актуальная сборка: files-ru.3

APK: **`gridcraft-0.5.0-arm64-v8a-files-ru3.apk`**, версия `0.5.0-files-ru.3`
(16778502). Открытие и вставка рисунка используют **исходное встроенное окно**.
Системный выбор документов не вызывается. Фильтр поддерживает все 14 расширений
движка, включая XLSX, XLTX/XLTM, XLS, XLSB, ODS и CSV; регистр и кириллица учитываются.
Открытые через встроенное окно книги сохраняют исходный путь.

Для общей памяти Android 11+ включите для GridCraft «Доступ ко всем файлам».
Окно проверяет фактическое разрешение, показывает закрытые папки и открывает настройки
доступа. После разрешения предпочитается папка с подходящими документами, затем «Загрузки».

«Открыть с помощью» принимает content/file URI с любым MIME-типом или без него,
а также передачу через «Поделиться». Входящие content URI открываются как копии;
для общей папки используйте «Сохранить как».

Завершён перевод команд, вложенных меню, панелей и диалогов; поиск команд работает
по-русски. 13 тестов оболочки и 65 тестов интерфейса проходят.
Подробности: [docs/BUILTIN-PICKER-RU3.md](docs/BUILTIN-PICKER-RU3.md).

Проверено на планшете HONOR MLA-W09 с Android 16: встроенный выбор, открытие из X-plore
и штатного менеджера «Файлы», холодный и повторный запуск. Финальный APK установлен
на устройство; его SHA-256 совпадает с локальным файлом.

Ключ подписи тот же, что у files-ru.2: её можно обновить без удаления.
Более ранние локальные APK подписаны иначе. Ключ для следующих обновлений сохранён
в `gridcraft-sideload.keystore` (не входит в Git).

Дополнительный патч перевода `0002-russian-dialogs.patch` автоматически применяется
скриптом `scripts/build-apk.ps1`. При ручной сборке примените его после `0001`.

Описание и результаты проверок ниже относятся к предыдущей локальной сборке
`files-ru.1`. Актуальные проверки на устройстве приведены в `docs/BUILTIN-PICKER-RU3.md`.

An unofficial Android port of **[GridCraft](https://github.com/storytold/gridcraft)** — the real engine
and egui interface, packaged as a NativeActivity APK for sideloading on arm64 devices.

| | |
|---|---|
| Upstream project | [`storytold/gridcraft`](https://github.com/storytold/gridcraft) — Apache-2.0 OR MIT |
| Pinned commit | `a513787` (upstream version `0.5.0`) |
| Android package | `ai.storyteller.gridcraft` |
| Local fixed APK (2026-11-13) | `gridcraft-0.5.0-arm64-v8a-files-ru.apk` — 11,625,340 bytes; version `0.5.0-files-ru.1`, code `16778500`; **new signing key** (uninstall the previous local APK first) |
| Local APK sha256 | `042fcc9329d96f79a97405fff041d1265e82bbb85770c5472fb9b0750538248c` |
| Published APK | `gridcraft-0.5.0-arm64-v8a-20261013b.apk` — 11.1 MB, arm64-v8a, signed (release **v0.1.2**, 2026-10-13) |
| APK sha256 | `930bec5b49369e12579d2b4e2f299f71df07fb28a7ce08932f1fc8d7005b645f` |
| Upstream files changed | 7 files (1 of them new) — see [`patches/`](patches/) |
| New port crate | `apps/gridcraft-android/` — the Rust shell, a Java Activity for permissions and *Open with*, and resources |

## What this repository is

This is **not a fork** of GridCraft. It holds only the Android layer, so it stays small, readable,
and obviously separable from upstream:

```
apps/gridcraft-android/     the new shell crate — copy it into your checkout of upstream
patches/                the diff against upstream — apply it after checking out the pin
docs/PORT-NOTES.md      what was changed and why, fix by fix, with how each one was checked
docs/FILE-ACCESS-FIX.md storage access, Russian picker and Android *Open with*, in one place
scripts/                cross-compile environment, build and verify scripts (from shared-android-port)
CHANGELOG.md            what is in the published APK
```

The Android layer is not a reimplementation. It is a shell crate that fills the same `Services`
struct the desktop and web shells fill, so the real engine and the real egui UI run unchanged.

## Build it

Prerequisites: Rust (rustup) with the `aarch64-linux-android` target, Android SDK + NDK r27, a
JDK 21 or newer, and `cargo install cargo-apk`.

```bash
git clone https://github.com/storytold/gridcraft.git
cd gridcraft
git fetch --depth 1 origin a5137872c15fac7bf260fa2d25ea8f7e66b19dc2
git checkout FETCH_HEAD                        # the exact commit this port targets
git apply -3 /path/to/0001-gridcraft-android-port.patch
cp -r /path/to/gridcraft-android/apps/gridcraft-android apps/   # the shell crate from this repository

# any keystore of your own signs an installable APK; a throwaway test key is fine for sideloading
keytool -genkeypair -keystore ks.jks -alias gridcraft -keyalg RSA -keysize 2048 -validity 10000 \
        -storepass android -keypass android -dname "CN=Local Test, O=Local, C=US"
export CARGO_APK_RELEASE_KEYSTORE="$PWD/ks.jks" CARGO_APK_RELEASE_KEYSTORE_PASSWORD=android
cargo apk build --release -p gridcraft-android  # native build stage only; complete with build-apk.ps1 below
```

`scripts/android-env.sh` is the cross-compile environment this port was built with (the NDK linker
and the C compiler the native dependencies need), `scripts/build-apk.sh` wraps the native build,
`scripts/build-apk.ps1` completes it with the Java Activity, `scripts/package-bridge.py` merges the
DEX and native library into the packaged APK, and `scripts/verify-apk.sh` checks the artifact:
`aapt2 dump badging`, `apksigner verify`, and `llvm-nm` on the `.so` inside the APK.

## Install

```bash
adb install -r gridcraft-0.5.0-arm64-v8a-files-ru.apk
```
or copy the APK to the device and open it (installing from unknown sources must be allowed).

The old local APK was signed with a different key, so Android refuses to update it: uninstall
`ai.storyteller.gridcraft` first (its files in the app's own folder go with it).

Open and Save As both use the built-in file picker, with Russian labels when the editor uses
Russian. On first use the app opens Android's storage permission UI on the main thread: all-files
access settings on Android 11+, or read/write permission on Android 8–10. Enable access and return
to GridCraft; the picker moves to shared Documents/Download. It also offers the internal-storage
root, available SD cards and a Show all files checkbox. Opening through this picker uses the
original file path, so ordinary Save updates the original workbook.

GridCraft is also registered in Android's **Open with / Открыть с помощью** chooser for spreadsheet
formats — XLSX/XLTX, XLSM/XLTM, XLSB, XLS, ODS, CSV/TSV, plain text and JSON — plus generic binary
documents. Choose *Always* in the chooser to make it the default. The Activity handles both cold
starts and new intents delivered to an existing window. Content-provider URIs are imported into
durable private storage on a worker; use Save As to save an externally supplied copy to a public
folder. Readable file URIs are opened directly. An incoming workbook opens in its own tab, so an
unsaved workbook is never closed behind the user's back.

**Build this version with the Windows script**, using the assembled upstream workspace described
above. A bare `cargo apk build` does not include the required Java Activity:

```powershell
./scripts/build-apk.ps1 -Workspace C:/path/to/assembled/gridcraft `
    -Sdk C:/Android/sdk -Keystore C:/path/to/gridcraft-sideload.keystore
```

Requires Python 3 and a JDK providing javac on PATH. The script builds Rust, compiles the Activity
with javac and D8, packages classes.dex and the Android 10 storage flag, aligns and signs the APK.
The default output is `gridcraft-0.5.0-arm64-v8a-files-ru.apk` in this repository.
The keystore password expected by the script is `android`.

## Verification status

For the local fixed APK on 2026-11-13: all 11 shell tests passed, including Russian picker
dialogs, Unicode/uppercase filenames, opening a workbook from the file manager without closing
unsaved work, and the asynchronous Open answer landing as a path. The patch applies to a clean
checkout of the pin, the assembled tree matches the build workspace file-by-file (17 files), and
the freshly assembled tree compiles for `aarch64-linux-android`. Checked in the APK: signatures
v2/v3, package/version/ABI, both native entry points, `classes.dex` with `onCreate`, `onNewIntent`,
`takeOpenedDocument` and `requestStorageAccess`, `hasCode`, `requestLegacyExternalStorage`,
`android.app.lib_name=gridcraft_android`, the ACTION_VIEW/CATEGORY_DEFAULT MIME filters, and the
`adjustResize` theme item. **Not tested on a physical device from here.**

Everything below was checked on the machine this port was published from, in October 2026.
A tick means the check was *run*, not inferred; the wording says how far it goes.

| Claim | How it was checked | Result |
|---|---|---|
| `patches/…patch` applies to a clean checkout of the pin | fresh `git init` + `git fetch --depth 1 origin a5137872c15fac7bf260fa2d25ea8f7e66b19dc2` + `git apply --check` | ✅ applies cleanly |
| The assembled tree equals the tree the APK was built from | file-by-file SHA-256 over every patched file and every file of the port crate | ✅ 17 files identical, none differing |
| The port crate compiles for Android | `cargo check -p gridcraft-android --target aarch64-linux-android`, run in a tree assembled from scratch (fresh `git fetch` of the pin, patch applied, port crate copied in) | ✅ exit 0 |
| AutoSum number-format and Format Painter fixes | `cargo test -p gridcraft-engine` (host) | ✅ 4/4 pass; the whole engine lib suite passes 207/207 |
| The port crate's own tests | `cargo test -p gridcraft-android` in the assembled tree | ✅ 11/11 pass (2026-11-13, current local build) |
| APK is signed and is the right artifact | `apksigner verify --verbose`, `aapt2 dump badging`, sha256 | ✅ v2/v3 signatures true, `native-code: 'arm64-v8a'`, package `ai.storyteller.gridcraft` |
| The Java Activity and *Open with* are packaged | `dexdump` on the APK's `classes.dex`; `aapt2 dump xmltree` on its manifest | ✅ `onCreate`, `onNewIntent`, `takeOpenedDocument` and `requestStorageAccess` present; `hasCode=true`; `VIEW`/`DEFAULT` MIME filters present |
| Behaviour on real hardware | hand-testing on the porter's arm64 tablet | ⚠️ the fixes below were found and driven by testing on this suite's tablet; this build has not been run on a device here |

**Before trusting the APK:** the source side is verified as listed above; runtime behaviour only as far
as [`docs/PORT-NOTES.md`](docs/PORT-NOTES.md) says per fix, plus the tablet testing that produced those
fixes. There is no automated device test in this repository.

## Known gaps

- **Runtime verification is hand-testing only.** The engine tests were run here (207/207), but no
  device test exists, and this particular APK was not run on a device on this machine.
- **The new local APK uses a new signing key**, so it cannot be installed over the earlier local
  build; uninstall that one first.
- **All files access is a manual grant.** The app asks for it by opening the system screen the
  first time Open or Save As appears (and again from the picker's banner or a padlocked folder),
  but the permission itself has no runtime dialog, so the user still has to flip the switch there.
- **A content-URI workbook is a copy.** Android hands over a temporary read grant, so the file is
  imported into the app's private storage; Save As is how it reaches a public folder. A `file:` URI
  is opened where it lies.
- **Format Painter applies on click and drag only.** There is no double-click-to-lock (sticky) mode
  beyond the engine's own one-shot behaviour.
- **`Ctrl` shortcuts work only because the shell synthesises modifiers** — if a device's IME swallows
  a key event entirely, that modifier cannot be seen.


## The rest of this set

- [photocraft-android](https://github.com/kanaaka/photocraft-android) — PhotoCraft 0.7.0
- [vectorcraft-android](https://github.com/kanaaka/vectorcraft-android) — VectorCraft 0.9.0
- [wordcraft-android](https://github.com/kanaaka/wordcraft-android) — WordCraft 0.5.0
- [pdfcraft-android](https://github.com/kanaaka/pdfcraft-android) — PdfCraft 0.6.0
- [deckcraft-android](https://github.com/kanaaka/deckcraft-android) — DeckCraft 0.5.0
- [shared-android-port](https://github.com/kanaaka/shared-android-port) — build/verify scripts + the cross-compile environment

## Credits and licence

GridCraft is © its authors at [`storytold/gridcraft`](https://github.com/storytold/gridcraft) and is
dual-licensed Apache-2.0 / MIT; both licence texts are kept here. The Android layer in this
repository is an unofficial modification by [@kanaaka](https://github.com/kanaaka), carrying the
same licence. Not affiliated with or endorsed by the upstream project. See
[`ATTRIBUTION.md`](ATTRIBUTION.md).
