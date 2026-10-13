//! Dostihy & Sázky – a clone of the Czech horse-racing board game.

// Bevy queries are verbose by nature.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

pub mod ai;
pub mod data;
pub mod original;
pub mod rules;
pub mod ui;

use bevy::asset::AssetMetaCheck;
use bevy::prelude::*;
use bevy::window::WindowResolution;

/// Raised by the UI while it animates; computer players wait for it.
#[derive(Resource, Default)]
pub struct Busy(pub bool);

/// The canvas the game draws on in a browser.
pub const CANVAS: &str = "#bevy-canvas-target";

/// The game's assets folder: `assets/`, in a browser next to the game's own
/// files (`dostihy.js`), wherever the page embedding it is.
pub fn asset_root() -> String {
    #[cfg(target_arch = "wasm32")]
    return MODULE_URL.with(|url| format!("{}assets", &url[..url.rfind('/').map_or(0, |i| i + 1)]));
    #[cfg(not(target_arch = "wasm32"))]
    "assets".into()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    /// `import.meta.url` of the JS glue: where the game's files are.
    #[wasm_bindgen(thread_local_v2, js_namespace = ["import", "meta"], js_name = url)]
    static MODULE_URL: String;
}

/// Default plugins with the game window and the original game's assets.
pub fn window_plugins() -> impl PluginGroup {
    DefaultPlugins
        .set(WindowPlugin {
            primary_window: Some(Window {
                title: "Dostihy & Sázky".into(),
                resolution: WindowResolution::new(1400, 880),
                // In a browser: the page's canvas, as big as the page lets it be.
                canvas: Some(CANVAS.into()),
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        })
        // There are no `.meta` files; in a browser each would be a request that 404s.
        .set(AssetPlugin { file_path: asset_root(), meta_check: AssetMetaCheck::Never, ..default() })
        .add(original::OriginalPlugin)
}
