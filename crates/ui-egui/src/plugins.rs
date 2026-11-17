//! The plugin store page: the shared plugin manager (`craft-ext-ui` over `craft-ext-manager`) for
//! every Craft app the launcher knows that hosts plugins. Desktop only: the store is a folder on
//! disk and the catalogue refresh goes to the network (`lib.rs` and `app.rs` keep the web build away
//! from this module).
//!
//! Invariants: the launcher is never required, since each app keeps its own manager and
//! `<app>-cli plugin`; a missing store folder is a plain error, never a guessed location. The
//! manager refreshes a stale catalogue itself and asks which app to install into when a plugin
//! supports several.

use artcraft_launcher_core::hosted_catalog;
use craft_ext_cli::known_apps;
use craft_ext_core::id::AppId;
use craft_ext_store::location;
use craft_ext_ui::craft_ext_manager::{AppTarget, Config, Word};
use craft_ext_ui::{PluginManager, Words};
use egui::{RichText, Ui};

use crate::app::LauncherUi;
use crate::i18n::fmt;
use crate::theme::{self, Tokens};

/// What the plugin store page keeps while the launcher runs: the view's tab, search and selection
/// and the manager model with its worker.
pub(crate) struct Plugins {
    view: PluginManager,
}

impl Plugins {
    /// Finds the store folder and manages every app in `apps` that hosts plugins. Fails with a plain
    /// message when no store folder can be found on this computer.
    fn open<'a>(apps: impl Iterator<Item = &'a str>) -> Result<Self, String> {
        let root = location::default_root().map_err(|e| e.to_string())?;
        let targets = apps.filter_map(|id| Some(AppTarget { app: AppId::new(id).ok()?, provided: known_apps::provided(id)? })).collect();
        Ok(Self { view: PluginManager::new(Config::new(root, targets, platform())) })
    }
}

/// The manager's words in the launcher's language (English where the catalog has no row) and the
/// apps by their display names.
struct LauncherWords;

impl Words for LauncherWords {
    fn word(&self, word: Word) -> String {
        crate::i18n::t(word.english()).to_string()
    }

    fn app(&self, app: &AppId) -> String {
        hosted_catalog::apps().iter().find(|a| a.id == app.as_str()).map_or_else(|| app.to_string(), |a| a.name.to_string())
    }
}

/// The platform key native plugins are listed under (`linux-x86_64`, `macos-aarch64`, …), the same
/// key `craft_ext_host::native::platform` gives.
fn platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

impl LauncherUi {
    /// The plugin store page. Leaving it keeps the state, so coming back returns to the same tab,
    /// search and plugin.
    pub(crate) fn plugins_page(&mut self, ui: &mut Ui) {
        let apps: Vec<&'static str> = self.entries.iter().map(|e| e.app.id).collect();
        let plugins = self.plugins.get_or_insert_with(|| Plugins::open(apps.into_iter()));
        match plugins {
            Ok(plugins) => plugins.view.show(ui, &LauncherWords),
            Err(why) => {
                let t = Tokens::get(ui.ctx());
                ui.label(RichText::new(tl!("Plugins")).size(18.0).family(theme::semibold()).color(t.ink_strong));
                ui.label(fmt(tl!("Plugins are unavailable: {error}"), &[("error", why.as_str())]));
            }
        }
    }
}
