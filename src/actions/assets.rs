//! Asset-loading helpers for E2E scenarios.
//!
//! Provides a generic `WaitUntil` action that blocks until specific assets
//! are fully loaded, useful when scenarios need assets that aren't covered
//! by the loading screen preloading.

use bevy::prelude::*;

use crate::action::Action;

/// Generic helper: wait until all the given asset paths are loaded
/// (including recursive dependencies like embedded textures/materials).
///
/// Uses untyped asset loading so it works for any asset type without
/// requiring specific Bevy features (e.g. `bevy_scene`).
///
/// # Example
///
/// ```rust,ignore
/// use bevy_e2e::actions::assets;
///
/// // Wait up to 600 frames (~10s) for custom models to load
/// .action(assets::wait_for_assets("my models", &["models/custom.glb#Scene0"], 600))
/// ```
pub fn wait_for_assets(label: &str, paths: &'static [&'static str], max_frames: u32) -> Action {
    let label = label.to_string();
    Action::WaitUntil {
        label,
        condition: Box::new(move |world: &World| {
            let asset_server = world.resource::<AssetServer>();
            paths.iter().all(|path| {
                // load_untyped() is idempotent — returns the cached handle if already loading/loaded
                let handle = asset_server.load_untyped(*path);
                asset_server.is_loaded_with_dependencies(&handle)
            })
        }),
        max_frames,
    }
}
