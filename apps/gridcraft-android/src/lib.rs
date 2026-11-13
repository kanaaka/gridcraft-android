//! GridCraft on Android (arm64-v8a): the real engine and egui shell, running on winit's
//! NativeActivity backend.
//!
//! # What this crate is
//!
//! A shell around the *unmodified* GridCraft crates — `gridcraft-engine` (the workbook model,
//! the formula engine, XLSX) and `gridcraft-ui-egui` (the ribbon, Backstage, grid, dialogs).
//! Nothing in those crates is patched. This crate supplies only what `apps/gridcraft` would
//! normally supply: the [`Services`] implementation, the preferences files, the window options
//! and the entry point. The same shape the PhotoCraft, VectorCraft and WordCraft ports used.
//!
//! # Why a separate crate, and not a feature on `apps/gridcraft`
//!
//! That crate cannot build for Android as written: it carries native dialogs (`rfd`), the OS
//! clipboard (`arboard`), a control server and a desktop appearance watcher. Verified with
//! `cargo check -p gridcraft --target aarch64-linux-android`: `arboard` stops first, with
//! `cannot find Set in module platform` — it has no Android backend at all — and `rfd` fails the
//! same way with `the trait bound FileDialog: FilePickerDialogImpl is not satisfied`.
//!
//! `gridcraft_ui_egui::Services` is a plain struct of `Option<Box<dyn Fn…>>` fields that derives
//! `Default`, so the fix is not to patch the app but to build a platform that supplies what it
//! can and omits the rest. Its `ui-egui` is Android-clean: `cargo check -p gridcraft-ui-egui
//! --target aarch64-linux-android` passes with no changes.
//!
//! # Which half of the seam this shell fills, and why it is not `pick_open`/`pick_save`
//!
//! GridCraft's dialogs come in two shapes. `pick_open` and `pick_save` are the *blocking* ones —
//! on the desktop they run `rfd` in line and return the chosen path immediately. That cannot
//! work here: a picker this shell has to *draw* needs the frame to end and come back, so a
//! function that must return a path now can never show one. The host patch therefore adds the
//! asynchronous shapes to `Services`, and this shell supplies those:
//!
//! - **Open / Save As** — `pick_open_async` and `pick_save_async` return a receiver, and the app
//!   opens or saves by *path* on the frame the answer lands. The workbook records its own file,
//!   so the title bar, Recent and Ctrl+S behave as on the desktop.
//! - **Insert Picture** — `pick_picture_async(inbox)` delivers `(name, bytes)`, which is what the
//!   picture command wants; each dialog owns its inbox.
//! - **Export** — `download(name, bytes)` is handed the finished file, exactly as the browser is.
//!   Here it asks for a path first and writes there, reporting failures in a notice.
//!
//! Supplying more than one seam is safe: the app prefers the path-based one and only falls back
//! to bytes when the host has no dialog at all.
//!
//! # What this shell adds to the app on Android
//!
//! - **Open and Save As are path-based**, through the upstream `pick_open_async` and
//!   `pick_save_async` seams: the workbook records the file it came from, the title bar and Recent
//!   follow, and Ctrl+S writes back to it without asking again.
//! - **Files reach the app from Android itself.** The Activity advertises the spreadsheet MIME
//!   types this engine reads, so GridCraft appears in *Open with* and can be made the default for
//!   them. A content URI is copied into the app's own storage on a worker (the read grant is
//!   temporary); a readable `file:` URI is opened where it lies.
//! - **Shared storage is asked for**, not merely padlocked: the picker opens the system screen that
//!   grants *All files access*, and follows the device's actual storage root (§ `permissions`).
//! - **Russian labels** in the picker and its dialogs follow the editor's interface language.
//!
//! # Feature notes
//!
//! - `android-native-activity` **must** be on. Without a backend feature, `android-activity` is
//!   pulled with no backend and the build fails with `file not found for module activity_impl`
//!   plus `Either "game-activity" or "native-activity" must be enabled as features`.
//! - `accesskit` **must** be off, and here it is unreachable: `ui-egui` has no eframe dependency
//!   at all, so this crate is the only eframe user in the graph and never asks for it.
//! - `NativeOptions::android_app` must be `Some(..)`: eframe hands it to
//!   `EventLoopBuilderExtAndroid::with_android_app` and returns an error without it.
//! - The APK needs the Java Activity this crate carries (`java/…/GridcraftActivity.java`), so it is
//!   packaged by `scripts/build-apk.ps1`, not by a bare `cargo apk build` — see the README.
//!
//! # What Android cannot do yet
//!
//! Honest gaps, not oversights:
//!
//! - **No system clipboard.** `arboard` has no Android backend, so `copy_html` and `paste_html`
//!   stay `None`: Cut/Copy/Paste of cells falls back to whatever egui itself can do, and the
//!   HTML flavour that keeps pasted numbers numeric is gone with it.
//! - **No opening links.** `open_url` stays `None`, so the Discord/website/GitHub links in the
//!   credits and the Backstage do nothing — that needs an `Intent` through JNI.
//! - **No printing.** `print` stays `None`; File ▸ Print reports what GridCraft's own fallback
//!   says rather than handing a PDF to a print service.
//! - **No stylus pressure or tilt.** A pen degrades to ordinary touch.
//! - **No system title bar.** The app draws its own, which is what the tablet shows, fullscreen.
//!
//! The workbook model, the formula engine, the grid and the XLSX writer are platform-neutral and
//! unaffected, and the interface fonts are embedded, so text works with no system font folder.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

#[cfg(target_os = "android")]
use gridcraft_engine::Session;
use gridcraft_ui_egui::{Inbox, Services, SheetApp, UiState};

#[cfg(target_os = "android")]
mod android_files;
mod file_picker;
mod modifiers;
mod permissions;
mod prefs_watch;
mod touch_scroll;

/// The app's private data directory, captured in `android_main` before any service closure runs:
/// `<internal storage>/Android/data/ai.storyteller.gridcraft/files`.
///
/// `internal_data_path()` needs no runtime permission — it is the app's own sandbox — which is
/// why the preferences and the log land here rather than in shared storage.
static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

// This window's `AndroidApp`, kept for what the app asks of the platform *after* start-up — the
// storage-permission screen and Android's *Open with* inbox, today.
//
// android-activity warns against holding an `AndroidApp` in global state because it dies with its
// Activity; this is per-thread state that `android_main` fills in and clears again as its loop
// runs and ends, so it never outlives the Activity it belongs to.
#[cfg(target_os = "android")]
thread_local! {
    static APP: std::cell::RefCell<Option<winit::platform::android::activity::AndroidApp>> =
        const { std::cell::RefCell::new(None) };
}

/// Run `f` with the JNI handles of this window's Activity.
///
/// `None` when there is no Activity to talk to: before `android_main` stored the handle, after
/// its window ended, and on the host build. The same rule as `APP` itself — per-thread state that
/// does not outlive the Activity it belongs to.
#[cfg(target_os = "android")]
pub(crate) fn with_android_app<R>(f: impl FnOnce(&winit::platform::android::activity::AndroidApp) -> R) -> Option<R> {
    APP.with(|slot| slot.borrow().as_ref().map(f))
}

/// The device's API level. Shared-storage access is a different permission before Android 11,
/// so [`permissions`] has to know which question to ask (see [`permissions::granted`]).
#[cfg(target_os = "android")]
fn android_sdk_version() -> i32 {
    winit::platform::android::activity::AndroidApp::sdk_version()
}

#[cfg(not(target_os = "android"))]
#[allow(dead_code)]
fn android_sdk_version() -> i32 {
    0
}

/// Messages the shell shows the user when work it started on a worker thread fails.
///
/// Reading a picked file and writing a Save happen off the frame, where nothing can raise a
/// dialog; without this a failed write to a folder the app may not touch would look like the
/// Save button doing nothing.
type Notices = Arc<Mutex<Vec<String>>>;

/// Mirrors the desktop `load_prefs`, with the settings directory replaced by the app's own.
fn load_prefs(app: &mut SheetApp) {
    let Some(dir) = DATA_DIR.get() else {
        return;
    };
    // First run: follow the device's language and appearance until the user chooses and saves.
    app.ui.language = gridcraft_ui_egui::i18n::Language::system();
    app.ui.system_theme = true;
    if let Ok(bytes) = std::fs::read(dir.join("ui.json"))
        && let Ok(ui) = serde_json::from_slice::<UiState>(&bytes)
    {
        let saved = serde_json::from_slice(&bytes).ok().and_then(|v| gridcraft_ui_egui::i18n::saved_language(&v));
        let language = saved.unwrap_or(app.ui.language);
        app.ui = ui;
        app.ui.language = language;
    }
    if let Ok(bytes) = std::fs::read(dir.join("prefs.json"))
        && let Ok(prefs) = serde_json::from_slice(&bytes)
    {
        app.session.prefs = prefs;
    }
}

/// How often the preferences are looked at; a change is written at the next look. Short enough
/// that nothing a user would miss is stale, long enough to cost nothing while the app is idle.
const PREFS_EVERY: Duration = Duration::from_secs(2);

/// What goes to `ui.json` — where the recent-documents list lives.
fn prefs_ui_bytes(app: &SheetApp) -> Vec<u8> {
    serde_json::to_vec_pretty(&app.ui).unwrap_or_default()
}

/// What goes to `prefs.json` — the engine's own preferences.
fn prefs_engine_bytes(app: &SheetApp) -> Vec<u8> {
    serde_json::to_vec_pretty(&app.session.prefs).unwrap_or_default()
}

/// The two files the desktop writes on its way out, in the app's own folder, which always exists.
fn prefs_files() -> (Option<PathBuf>, Option<PathBuf>) {
    let Some(dir) = DATA_DIR.get() else {
        return (None, None);
    };
    let _ = std::fs::create_dir_all(dir);
    (Some(dir.join("ui.json")), Some(dir.join("prefs.json")))
}

/// Write the preferences if they changed, at most every [`PREFS_EVERY`] (see [`prefs_watch`]).
fn save_prefs_if_due(app: &SheetApp, ui: &mut prefs_watch::Writer, engine: &mut prefs_watch::Writer) {
    if ui.due() {
        let bytes = prefs_ui_bytes(app);
        ui.store(&bytes);
    }
    if engine.due() {
        let bytes = prefs_engine_bytes(app);
        engine.store(&bytes);
    }
}


/// The workbooks Open offers, as on the desktop.
fn workbook_extensions() -> Option<Vec<String>> {
    file_picker::extensions(&[&["xlsx", "xlsm", "xls", "xlsb", "ods", "csv", "tsv", "txt", "json"]])
}

fn picture_extensions() -> Option<Vec<String>> {
    file_picker::extensions(&[&["png", "jpg", "jpeg"]])
}

/// Wait for a pick off the UI thread, read what it names and hand it to the app the way a
/// browser file input does: `(name, bytes)` pushed into the inbox the app polls each frame.
///
/// This is the picture seam, whose answer is bytes; a workbook is opened by path through
/// [`Services::pick_open_async`] instead, so it knows its own file.
fn deliver(inbox: Inbox, ctx: egui::Context, picks: Receiver<Vec<PathBuf>>, notices: Notices) {
    let reading = std::thread::Builder::new().name("file read".into()).spawn(move || {
        let Ok(paths) = picks.recv() else {
            return;
        };
        let mut arrived = false;
        for path in paths {
            match std::fs::read(&path) {
                Ok(bytes) => {
                    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    log::info!("read {} ({} bytes)", path.display(), bytes.len());
                    inbox.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push((name, bytes));
                    arrived = true;
                }
                Err(e) => {
                    log::error!("couldn't read {}: {e}", path.display());
                    notices.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(format!(
                        "{} {}: {e}",
                        file_picker::text("Couldn't read", "Не удалось прочитать"),
                        path.display()
                    ));
                }
            }
        }
        // The app is idle while it waits; wake it to use what arrived.
        if arrived {
            ctx.request_repaint();
        }
    });
    if let Err(e) = reading {
        log::error!("couldn't start the file-reading thread: {e}");
    }
}

/// Write the bytes the app handed to `download`, once the user has named a folder and file.
///
/// This is GridCraft's browser save path landing on a real filesystem: it asks where, then
/// writes there. The app's own folder (what Save suggests first) always works with no
/// permission at all; anywhere else under shared storage needs "All files access".
fn save(ctx: egui::Context, picks: Receiver<Vec<PathBuf>>, bytes: Vec<u8>, notices: Notices) {
    let writing = std::thread::Builder::new().name("file write".into()).spawn(move || {
        let Ok(paths) = picks.recv() else {
            return;
        };
        let Some(path) = paths.into_iter().next() else {
            return;
        };
        match std::fs::write(&path, &bytes) {
            Ok(()) => log::info!("wrote {} ({} bytes)", path.display(), bytes.len()),
            Err(e) => {
                log::error!("couldn't write {}: {e}", path.display());
                notices.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(format!(
                    "{} {}: {e}",
                    file_picker::text("Couldn't write", "Не удалось записать"),
                    path.display()
                ));
            }
        }
        ctx.request_repaint();
    });
    if let Err(e) = writing {
        log::error!("couldn't start the file-writing thread: {e}");
    }
}

/// The Android `Services`: the asynchronous half of the desktop's, plus the picker.
///
/// Everything the desktop fills with a blocking dialog or a system integration stays `None`,
/// and that is deliberate on every field — see the module docs.
fn android_services(ctx: egui::Context, notices: Notices) -> Services {
    let picture_ctx = ctx.clone();
    let picture_notices = notices.clone();
    let save_ctx = ctx.clone();
    let save_notices = notices.clone();

    Services {
        // File ▸ Open: a dialog that answers between frames, with the *path* of the workbook. The
        // app opens that file itself, so the title bar, Recent and Ctrl+S know which file it is.
        pick_open_async: Some(Box::new(|| {
            let title = file_picker::text("Open \u{2014} Android", "Открыть \u{2014} Android");
            let picks = file_picker::request(file_picker::Kind::Open, title, workbook_extensions(), None);
            path_answer(picks)
        })),
        // File ▸ Save As: a save dialog that answers between frames, so the workbook records the
        // file the user chose. Handing the app's own `download` its bytes instead (below) writes
        // the file and tells the app nothing, which is why the title stayed "Book1" and closing
        // asked to save what had just been saved.
        pick_save_async: Some(Box::new(move |suggested: &str| {
            let title = file_picker::text("Save As \u{2014} Android", "Сохранить как \u{2014} Android");
            let picks = file_picker::request(file_picker::Kind::Save, title, None, Some(suggested.to_string()));
            path_answer(picks)
        })),
        // Insert ▸ Pictures: its own inbox, so a cancelled pick cannot land in a later dialog.
        pick_picture_async: Some(Box::new(move |inbox: Inbox| {
            let title = file_picker::text("Insert Picture \u{2014} Android", "Вставить рисунок \u{2014} Android");
            let picks = file_picker::request(file_picker::Kind::Open, title, picture_extensions(), None);
            deliver(inbox, picture_ctx.clone(), picks, picture_notices.clone());
        })),
        // File ▸ Save As, and every Backstage export: ask where, then write the bytes there.
        download: Some(Box::new(move |name: &str, bytes: &[u8]| {
            let title = file_picker::text("Save As \u{2014} Android", "Сохранить как \u{2014} Android");
            let picks = file_picker::request(file_picker::Kind::Save, title, None, Some(name.to_string()));
            save(save_ctx.clone(), picks, bytes.to_vec(), save_notices.clone());
        })),
        ..Default::default()
    }
}

/// Open a workbook delivered by Android: the same path through the app the desktop's Open uses,
/// so Recent, the title bar and Save all see the file.
#[cfg(any(target_os = "android", test))]
fn open_external_document(app: &mut SheetApp, path: String) {
    // A file manager's document is what the user asked to see; the Backstage would hide it.
    app.backstage = None;
    app.open_path(&path);
}

/// The picker answers with paths; a save seam speaks of one path, or nothing.
///
/// One short-lived thread per dialog, which is the shape the desktop shell has too: it waits for
/// `rfd`'s future off the UI thread while the frame carries on, and this waits for the picker's
/// answer the same way. A path-less answer is a cancellation, which is what the app reads it as.
fn path_answer(picks: Receiver<Vec<PathBuf>>) -> Receiver<Option<String>> {
    let (answer, rx) = std::sync::mpsc::channel();
    let waiting = std::thread::Builder::new().name("save dialog".into()).spawn(move || {
        let path = picks.recv().ok().and_then(|paths| paths.into_iter().next()).map(|p| p.to_string_lossy().into_owned());
        let _ = answer.send(path);
    });
    if let Err(e) = waiting {
        // The dropped sender reads as cancelled, which is the honest outcome.
        log::error!("couldn't start the save-dialog thread: {e}");
    }
    rx
}

/// A minimal file logger, so a sideloaded APK leaves a readable trace.
///
/// `logcat` is only reachable over adb; a file in the app's own data directory can be pulled with
/// any file manager, which is what a sideloaded build needs.
struct FileLogger {
    sink: Mutex<Option<std::fs::File>>,
}

impl log::Log for FileLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }
    fn log(&self, record: &log::Record) {
        if let Ok(mut guard) = self.sink.lock() {
            if let Some(f) = guard.as_mut() {
                use std::io::Write;
                let _ = writeln!(f, "[{}] {} {}", record.level(), record.target(), record.args());
            }
        }
    }
    fn flush(&self) {}
}

static LOGGER: OnceLock<FileLogger> = OnceLock::new();

fn init_logging(dir: &Path) {
    let Ok(file) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("gridcraft.log")) else {
        return;
    };
    let logger = LOGGER.get_or_init(|| FileLogger { sink: Mutex::new(None) });
    if let Ok(mut guard) = logger.sink.lock() {
        *guard = Some(file);
    }
    if log::set_logger(logger).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
}

/// The Android entry point.
///
/// `android-activity` spawns a dedicated thread for this and calls it with the activity handle.
/// `#[unsafe(no_mangle)]` with the Rust ABI is the required shape (edition 2024 requires the
/// `unsafe(..)` wrapper around `no_mangle`), and it is the symbol the injected NativeActivity
/// looks for.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(app: winit::platform::android::activity::AndroidApp) {
    // Count this window while its loop runs (see `prefs_watch`).
    let _window = prefs_watch::counted_window();
    // Kept for the platform actions that come later: the storage-permission screen and Android's
    // file inbox (see `permissions` and `android_files`). Cleared when this window's loop ends.
    APP.with(|slot| *slot.borrow_mut() = Some(app.clone()));
    let data_dir = app.internal_data_path().unwrap_or_else(|| PathBuf::from("."));
    let _ = std::fs::create_dir_all(&data_dir);
    let _ = DATA_DIR.set(data_dir.clone());
    init_logging(&data_dir);
    log::info!("GridCraft {} starting on Android; data dir {}", env!("CARGO_PKG_VERSION"), data_dir.display());
    // The app's external files directory: always readable and writable with no permission, so the
    // picker offers it as a root and Save suggests it first (see `file_picker`).
    if let Some(ext) = app.external_data_path() {
        let _ = std::fs::create_dir_all(&ext);
        let _ = file_picker::EXT_DIR.set(ext);
    }
    // Fonts: a folder the user can drop a font file into over USB, pointed at where this app's font
    // scan looks (see `file_picker::register_fonts`).
    file_picker::register_fonts();
    // Shared storage: say what the app can see, so a log pulled from the tablet explains a picker
    // that only offers its own folder (see `permissions`).
    log::info!(
        "shared-storage access: {}",
        if permissions::granted() { "granted" } else { "not granted — the picker will offer to ask" }
    );

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("GridCraft").with_app_id("ai.storyteller.gridcraft"),
        // Required on Android: eframe hands this to
        // `EventLoopBuilderExtAndroid::with_android_app` and errors without it.
        android_app: Some(app),
        ..Default::default()
    };

    let result = eframe::run_native(
        "GridCraft",
        options,
        Box::new(|cc| {
            let notices: Notices = Notices::default();
            let mut app = SheetApp::new(Session::new(), android_services(cc.egui_ctx.clone(), notices.clone()));
            load_prefs(&mut app);
            if let Some(rs) = &cc.wgpu_render_state {
                let info = rs.adapter.get_info();
                log::info!("wgpu adapter: {info:?}");
            } else {
                log::warn!("no wgpu render state");
            }
            let (ui_path, engine_path) = prefs_files();
            Ok(Box::new(Shell {
                inner: app,
                notices,
                modifiers: modifiers::Tracked::default(),
                touch: touch_scroll::TouchScroll::default(),
                prefs_ui: prefs_watch::Writer::new(ui_path, PREFS_EVERY),
                prefs_engine: prefs_watch::Writer::new(engine_path, PREFS_EVERY),
            }))
        }),
    );

    match result {
        Ok(()) => log::info!("GridCraft exited normally"),
        Err(e) => log::error!("GridCraft stopped: {e}"),
    }
    APP.with(|slot| *slot.borrow_mut() = None);
}

/// Wraps the real app so the platform can act on the frame around it: the picker, and the reports
/// of work that failed off the frame.
///
/// The same shape as the desktop shell (`apps/gridcraft/src/main.rs`) and the web build:
/// `SheetApp` stays untouched and the platform-specific layer hangs off the outside. Every method
/// is forwarded, so the app sees no difference.
struct Shell {
    inner: SheetApp,
    notices: Notices,
    /// Modifier keys, tracked here because the platform never reports them: winit's Android
    /// backend sends no `ModifiersChanged` at all, which is why Shift+click and every Ctrl
    /// shortcut are dead without this (see `modifiers`).
    modifiers: modifiers::Tracked,
    /// Two-finger scrolling, translated from touches into wheel events (see [`touch_scroll`]).
    touch: touch_scroll::TouchScroll,
    /// The preferences, written as the app runs rather than only on the way out: Android never
    /// gives that ending, so a swipe from Recents would otherwise take every preference with it
    /// (see [`prefs_watch`]). One writer per file, because they are two files.
    prefs_ui: prefs_watch::Writer,
    prefs_engine: prefs_watch::Writer,
}

impl Shell {
    /// Anything the picker's threads could not do, said out loud. Taking them one at a time means
    /// a repeated failure cannot pile up an unbounded list.
    fn show_notices(&mut self, ctx: &egui::Context) {
        let mut pending = self.notices.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(text) = pending.pop() else {
            return;
        };
        if !pending.is_empty() {
            pending.truncate(1);
        }
        drop(pending);
        egui::Window::new("GridCraft").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0)).show(ctx, |ui| {
            ui.spacing_mut().button_padding = egui::vec2(10.0, 8.0);
            ui.label(text);
            if ui.button(file_picker::text("OK", "ОК")).clicked() {
                *self.notices.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Vec::new();
            }
        });
    }
}

impl eframe::App for Shell {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // The picker's labels and the dialog titles the services ask for follow the editor's
        // language; the app sets it once per frame, the shell has to do it before the app runs.
        file_picker::set_language(self.inner.ui.language);
        self.inner.logic(ctx);
        #[cfg(target_os = "android")]
        {
            // A file manager may deliver a workbook during startup or to an existing Activity.
            // Open it through the app's normal path, so Recent, the title bar and Save see it.
            if !file_picker::is_open() && self.inner.dialog.is_none() && !self.inner.save_dialog_open()
                && let Some(path) = android_files::take_opened_document()
            {
                open_external_document(&mut self.inner, path);
            }
            // The inbox is filled by a Java worker; keep looking for it while idle.
            ctx.request_repaint_after(Duration::from_millis(500));
        }
        save_prefs_if_due(&self.inner, &mut self.prefs_ui, &mut self.prefs_engine);
    }

    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        // Before the app, so the frame it sees already has the modifiers a desktop backend would
        // have reported (see `modifiers`).
        let modifiers = self.modifiers.apply(raw);
        // Then the touches: winit's Android backend has no mouse and so sends no wheel events at
        // all, which is why nothing scrolled. A two-finger drag becomes one here (see
        // `touch_scroll`).
        self.touch.apply(raw, modifiers);
        self.inner.raw_input_hook(raw);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Before the app: a click that opens a dialog asks the picker for a title in this frame's
        // language.
        file_picker::set_language(self.inner.ui.language);
        self.inner.ui(ui);
        // After the app, so the picker and the report float above the grid.
        file_picker::draw(ui.ctx());
        self.show_notices(ui.ctx());
    }

    fn on_exit(&mut self) {
        let (ui_bytes, engine_bytes) = (prefs_ui_bytes(&self.inner), prefs_engine_bytes(&self.inner));
        self.prefs_ui.store_now(&ui_bytes);
        self.prefs_engine.store_now(&engine_bytes);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    /// A workbook on disk, produced by the app itself, for the tests that need a real file.
    fn workbook_file(tag: &str) -> (std::path::PathBuf, String) {
        let mut app = gridcraft_ui_egui::SheetApp::new(gridcraft_engine::Session::new(), gridcraft_ui_egui::Services::default());
        app.session.new_workbook();
        let answer = app.run("file.saveBytes", json!({"format": "xlsx"})).unwrap();
        let bytes = gridcraft_engine::io::base64_decode(answer["base64"].as_str().unwrap()).unwrap();
        let path = std::env::temp_dir().join(format!(
            "gridcraft-{tag}-{}-{}.xlsx",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::write(&path, bytes).unwrap();
        let text = path.to_string_lossy().into_owned();
        (path, text)
    }

    #[test]
    fn external_open_shows_the_file_and_keeps_unsaved_work() {
        let mut app = super::SheetApp::new(gridcraft_engine::Session::new(), super::android_services(egui::Context::default(), super::Notices::default()));
        app.session.new_workbook();
        app.run("cell.set", json!({"cell": "A1", "input": "Unsaved work"})).unwrap();
        let unsaved = app.session.active().unwrap().uid;
        assert!(app.session.active().unwrap().is_dirty());

        let (path, text) = workbook_file("external-open");
        super::open_external_document(&mut app, text.clone());
        assert_eq!(app.session.documents().len(), 2, "the incoming workbook opens in a tab of its own");
        assert!(app.session.documents().iter().any(|d| d.uid == unsaved && d.is_dirty()), "the unsaved workbook must survive");
        assert_eq!(app.session.active().unwrap().path.as_deref(), Some(text.as_str()), "the opened workbook knows its own file");
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn asynchronous_open_answers_by_path() {
        // A host that answers the Open dialog on a later frame, as the Android picker does.
        let mut app = super::SheetApp::new(gridcraft_engine::Session::new(), gridcraft_ui_egui::Services::default());
        app.session.new_workbook();
        let sender: std::sync::Arc<std::sync::Mutex<Option<std::sync::mpsc::Sender<Option<String>>>>> = Default::default();
        let host = sender.clone();
        app.services.pick_open_async = Some(Box::new(move || {
            let (tx, rx) = std::sync::mpsc::channel();
            *host.lock().unwrap() = Some(tx);
            rx
        }));
        let (path, text) = workbook_file("async-open");
        app.open_dialog("open", json!({}));
        sender.lock().unwrap().take().unwrap().send(Some(text.clone())).unwrap();
        app.logic(&egui::Context::default());
        assert_eq!(app.session.active().unwrap().path.as_deref(), Some(text.as_str()));
        std::fs::remove_file(path).ok();
    }
}
