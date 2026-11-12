//! The Plugins dialog: the shared plugin manager (`craft-ext-ui`) opened from the header, showing
//! every Craft app's plugins. Desktop only: the store is a folder on disk and the catalogue refresh
//! goes to the network (`lib.rs` and `views.rs` keep the web build away from this module).
//!
//! Invariants: the launcher is never required, since each app keeps its own manager and
//! `<app>-cli plugin`; the catalogue refresh runs in the background once per session, on the first
//! open; a missing store folder is a plain error, never a guessed location.

use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::time::Duration;

use craft_ext_cli::known_apps;
use craft_ext_core::{contract::Provided, id::AppId};
use craft_ext_store::{Manager, Store, location, store};
use craft_ext_ui::ManagerView;
use egui::{RichText, Ui};

use crate::app::LauncherUi;
use crate::i18n::fmt;
use crate::theme::{self, Tokens};
use crate::widgets::{self, Kind};

/// What the Plugins dialog keeps between openings: the view's search and tab, the all-apps manager
/// the view browses with, and the first-open catalogue refresh.
pub(crate) struct Plugins {
    view: ManagerView,
    /// Browses every app. Per-app actions build their own manager through [`app_manager`].
    mgr: Arc<Manager>,
    /// The first-open catalogue refresh, until it reports.
    refresh: Option<mpsc::Receiver<Result<(), String>>>,
    /// How the last refresh ended, shown until the next one.
    refresh_error: Option<String>,
}

impl Plugins {
    /// Finds the store folder and starts the catalogue refresh in the background. Fails with a plain
    /// message when no store folder can be found on this computer.
    pub(crate) fn open() -> Result<Self, String> {
        let root = location::default_root().map_err(|e| e.to_string())?;
        let mgr = Arc::new(Manager::new(Store::at(root), None, Provided::default(), platform()));
        let (tx, rx) = mpsc::channel();
        let background = Arc::clone(&mgr);
        std::thread::spawn(move || {
            // The dialog may be gone by the time this finishes; the result is then simply dropped.
            let _ = tx.send(background.refresh(store::now()).map_err(|e| e.to_string()));
        });
        Ok(Self { view: ManagerView::new_launcher(app_manager), mgr, refresh: Some(rx), refresh_error: None })
    }

    /// Takes the refresh's result once it has arrived; does nothing while it runs.
    fn poll_refresh(&mut self) {
        let Some(rx) = self.refresh.as_ref() else { return };
        match rx.try_recv() {
            Ok(result) => {
                self.refresh_error = result.err();
                self.refresh = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.refresh_error = Some("the background refresh stopped".into());
                self.refresh = None;
            }
        }
    }
}

/// The manager for one app, which the shared view asks for through a plain function (it cannot
/// capture anything), so it finds the store folder again. If the folder cannot be found, the store
/// is one no disk call can reach, and every operation fails instead of touching a guessed folder.
fn app_manager(app: &AppId) -> Manager {
    let store = match location::default_root() {
        Ok(root) => Store::at(root),
        Err(_) => unreachable_store(),
    };
    // An app this launcher has no contract table for gets none: its plugins show as unavailable.
    let provided = known_apps::provided(app.as_str()).unwrap_or_default();
    Manager::new(store, Some(app.clone()), provided, platform())
}

/// A store folder whose name holds a NUL byte. Every platform's file API refuses such a path, so
/// nothing is read or written.
fn unreachable_store() -> Store {
    Store::at(PathBuf::from("\0"))
}

/// The platform key native plugins are listed under (`linux-x86_64`, `macos-aarch64`, …), the same
/// key `craft_ext_host::native::platform` gives.
fn platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

impl LauncherUi {
    /// The Plugins dialog. Closing keeps the state, so reopening returns to the same search and tab.
    pub(crate) fn plugins_dialog(&mut self, ui: &mut Ui) -> bool {
        let t = Tokens::get(ui.ctx());
        if self.plugins.is_none() {
            match Plugins::open() {
                Ok(plugins) => self.plugins = Some(plugins),
                Err(why) => {
                    ui.label(RichText::new(tl!("Plugins")).size(18.0).family(theme::semibold()).color(t.ink_strong));
                    ui.label(fmt(tl!("Plugins are unavailable: {error}"), &[("error", why.as_str())]));
                    return widgets::button(ui, tl!("Close"), Kind::Secondary, 90.0).clicked();
                }
            }
        }
        let Some(plugins) = self.plugins.as_mut() else { return true };
        plugins.poll_refresh();
        if plugins.refresh.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        let width = 880.0f32.min(ui.ctx().content_rect().width() - 80.0);
        let max_height = ui.ctx().content_rect().height() * 0.8;
        ui.set_width(width);
        let mut close = false;
        egui::ScrollArea::vertical().id_salt("plugins-body").max_height(max_height).auto_shrink([false, true]).show(ui, |ui| {
            egui::Frame::new().inner_margin(egui::Margin::same(theme::PAD as i8)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(tl!("Plugins")).size(18.0).family(theme::semibold()).color(t.ink_strong));
                ui.add_space(8.0);
                if plugins.refresh.is_some() {
                    ui.label(RichText::new(tl!("Updating the plugin catalogue…")).color(t.muted));
                } else if let Some(why) = &plugins.refresh_error {
                    ui.label(fmt(tl!("The plugin catalogue could not be updated: {error}"), &[("error", why.as_str())]));
                }
                ui.add_space(8.0);
                plugins.view.show(ui, &plugins.mgr);
                ui.add_space(16.0);
                if widgets::button(ui, tl!("Close"), Kind::Secondary, 90.0).clicked() {
                    close = true;
                }
            });
        });
        close
    }
}
