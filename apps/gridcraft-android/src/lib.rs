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
//! function that must return a path now can never show one. Leaving both `None` is what makes
//! the app take its other branch, the one it already has for hosts that cannot block — the
//! browser build's:
//!
//! - **Open / Insert Picture** — `open_async` and `pick_picture_async` start the pick, and the
//!   file arrives later through `Services::inbox` as `(name, bytes)`, exactly as a browser
//!   file input delivers it. The app opens or inserts it on the next frame.
//! - **Save As / Export** — `download(name, bytes)` is handed the finished file, exactly as the
//!   browser is. Here it asks for a path first and writes there.
//!
//! So every dialog is reachable, and the only visible difference from the desktop is that Save
//! asks for the folder (the app is not the one holding a path afterwards — see "Gaps").
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
//!
//! # What Android cannot do yet
//!
//! Honest gaps, not oversights:
//!
//! - **A saved workbook does not learn its own path.** The desktop's `pick_save` hands the path
//!   back to the app, which records it, so the next Ctrl+S writes silently to the same file.
//!   This shell is given the bytes instead (that is the seam), so the write happens here and the
//!   session keeps `path = None`: the title bar stays "Book1" and the next Save asks again. The
//!   bytes written are the document's, so nothing is lost — it is the remembering that is
//!   missing. Anything else would have to guess which document was active when the user
//!   answered, and saving the wrong workbook over a file is worse than asking twice.
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

use gridcraft_engine::Session;
use gridcraft_ui_egui::{Inbox, Services, SheetApp, UiState};

mod file_picker;
mod modifiers;
mod prefs_watch;

/// The app's private data directory, captured in `android_main` before any service closure runs:
/// `<internal storage>/Android/data/ai.storyteller.gridcraft/files`.
///
/// `internal_data_path()` needs no runtime permission — it is the app's own sandbox — which is
/// why the preferences and the log land here rather than in shared storage.
static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

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
/// The desktop's `pick_open` returns a path instead, and `open_async` exists precisely for hosts
/// that cannot — on the web the bytes come from a `File` handle, here from the filesystem.
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
                    notices.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(format!("Couldn't read {}: {e}", path.display()));
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
                notices.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(format!("Couldn't write {}: {e}", path.display()));
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
    let inbox: Inbox = Inbox::default();

    let open_inbox = inbox.clone();
    let open_ctx = ctx.clone();
    let open_notices = notices.clone();
    let picture_ctx = ctx.clone();
    let picture_notices = notices.clone();
    let save_ctx = ctx.clone();
    let save_notices = notices.clone();

    Services {
        // File ▸ Save As: a save dialog that answers between frames, so the workbook records the
        // file the user chose. Handing the app's own `download` its bytes instead (below) writes
        // the file and tells the app nothing, which is why the title stayed "Book1" and closing
        // asked to save what had just been saved.
        pick_save_async: Some(Box::new(move |suggested: &str| {
            let picks = file_picker::request(file_picker::Kind::Save, "Save As \u{2014} Android", None, Some(suggested.to_string()));
            path_answer(picks)
        })),
        // File ▸ Open: the picked workbook arrives through the inbox.
        open_async: Some(Box::new(move || {
            let picks = file_picker::request(file_picker::Kind::Open, "Open \u{2014} Android", workbook_extensions(), None);
            deliver(open_inbox.clone(), open_ctx.clone(), picks, open_notices.clone());
        })),
        // Insert ▸ Pictures: its own inbox, so a cancelled pick cannot land in a later dialog.
        pick_picture_async: Some(Box::new(move |inbox: Inbox| {
            let picks = file_picker::request(file_picker::Kind::Open, "Insert Picture \u{2014} Android", picture_extensions(), None);
            deliver(inbox, picture_ctx.clone(), picks, picture_notices.clone());
        })),
        // File ▸ Save As, and every Backstage export: ask where, then write the bytes there.
        download: Some(Box::new(move |name: &str, bytes: &[u8]| {
            let picks = file_picker::request(file_picker::Kind::Save, "Save As \u{2014} Android", None, Some(name.to_string()));
            save(save_ctx.clone(), picks, bytes.to_vec(), save_notices.clone());
        })),
        inbox: Some(inbox),
        ..Default::default()
    }
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
                prefs_ui: prefs_watch::Writer::new(ui_path, PREFS_EVERY),
                prefs_engine: prefs_watch::Writer::new(engine_path, PREFS_EVERY),
            }))
        }),
    );

    match result {
        Ok(()) => log::info!("GridCraft exited normally"),
        Err(e) => log::error!("GridCraft stopped: {e}"),
    }
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
            if ui.button("OK").clicked() {
                *self.notices.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Vec::new();
            }
        });
    }
}

impl eframe::App for Shell {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.inner.logic(ctx);
        save_prefs_if_due(&self.inner, &mut self.prefs_ui, &mut self.prefs_engine);
    }

    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        // Before the app, so the frame it sees already has the modifiers a desktop backend would
        // have reported (see `modifiers`).
        self.modifiers.apply(raw);
        self.inner.raw_input_hook(raw);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
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
