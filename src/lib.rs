//! Skills: earn skill experience by playing, share it with friends nearby, and spend points in
//! skill trees on permanent perks. See `docs/design.md`.
//!
//! - **Data** (`tree.rs`): trees from `data/<ns>/skills/*.json`, merged and checked on each reload.
//! - **Progress** (`progress.rs`): per player, the only stored state (`store.rs`); levels, points
//!   and node states derive from it.
//! - **Earning** (`sources.rs`, `share.rs`): one statistic hook and one block hook turn actions
//!   into tree experience; friends nearby earn a share.
//! - **Perks** (`perks.rs`): attribute modifiers derived from progress, reconciled on the player.
//! - **Screen and toast** (`look.rs`; drawing in the build stage): the advancements screen's look.
mod look;
pub mod payloads;
pub mod perks;
pub mod progress;
pub mod share;
pub mod sources;
mod store;
pub mod tree;

use mod_api::{
    ClientHost, ClientTickPreEvent, GuiLayer, GuiLayerDrawEvent, GuiLayerPlacement, Hooks,
    IPayloadContext, KeyRegistration, PayloadId, RegisterGuiLayersEvent, Registrar,
    RegistrationContext, SettingSpec,
};
#[allow(unused_imports)]
use mod_api::prelude::*;
use std::sync::Mutex;

pub(crate) const MOD_ID: &str = "skills";
const SCREEN_KEY: &str = "open";
/// The HUD layer the level-up toast draws in.
const LAYER: &str = "skills:toast";

/// Settings (Options > Mods > Skills).
mod setting {
    pub const SHARE_RADIUS: &str = "share_radius";
    pub const SHARE_FRACTION: &str = "share_fraction";
    pub const TOASTS: &str = "toasts";
}

/// What the client knows: the server's trees and this player's progress, as last sent.
#[derive(Default)]
pub(crate) struct Seen {
    pub trees: tree::Trees,
    pub progress: progress::Progress,
    /// Whether the server has Skills (it sent trees).
    pub served: bool,
}

pub(crate) static SEEN: Mutex<Option<Seen>> = Mutex::new(None);

pub(crate) fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Client: the server's trees arrived.
fn on_trees(sync: payloads::TreesSync, _: &mut dyn IPayloadContext) -> Result<(), String> {
    let trees = serde_json::from_str(&sync.json).map_err(|e| e.to_string())?;
    let mut seen = lock(&SEEN);
    let seen = seen.get_or_insert_with(Seen::default);
    seen.trees = trees;
    seen.served = true;
    Ok(())
}

/// Client: this player's progress arrived (build stage: a level gained queues a toast).
fn on_progress(sync: payloads::ProgressSync, _: &mut dyn IPayloadContext) -> Result<(), String> {
    let progress = serde_json::from_str(&sync.json).map_err(|e| e.to_string())?;
    lock(&SEEN).get_or_insert_with(Seen::default).progress = progress;
    Ok(())
}

/// Server: a click on a node (build stage: check with `Progress::unlock`, reconcile perks, answer
/// with `ProgressSync`).
fn on_unlock(_: payloads::Unlock, _: &mut dyn IPayloadContext) -> Result<(), String> {
    Ok(())
}

#[derive(Default)]
pub struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    /// The screen key, the settings and the three payloads.
    fn register(&self, r: &mut dyn Registrar, _cx: &RegistrationContext) {
        // K: free in vanilla, Team (V) and Map (H, M, N, R).
        let mut key = KeyRegistration::new(SCREEN_KEY, "key.keyboard.k");
        key.label = "key.skills.open".into();
        r.register_key_mapping(key);
        for spec in [
            SettingSpec::range(setting::SHARE_RADIUS, "skills.config.share_radius", 0.0, 128.0, 1.0, 32.0),
            SettingSpec::range(setting::SHARE_FRACTION, "skills.config.share_fraction", 0.0, 1.0, 0.05, 0.5),
            SettingSpec::toggle(setting::TOASTS, "skills.config.toasts", true),
        ] {
            let _ = r.register_setting(spec);
        }
        let mut handlers = r.payload_handlers();
        let mut view = handlers.registrar("1").optional();
        let _ = payloads::TREES.set(view.play_to_client_typed(PayloadId("skills:trees".into()), on_trees));
        let _ = payloads::PROGRESS.set(view.play_to_client_typed(PayloadId("skills:progress".into()), on_progress));
        let _ = payloads::UNLOCK.set(view.play_to_server_typed(PayloadId("skills:unlock".into()), on_unlock));
    }

    fn register_gui_layers(&self, _: &RegistrationContext, event: &mut RegisterGuiLayersEvent<'_>) {
        event.layers.push(GuiLayer { id: LAYER.into(), placement: GuiLayerPlacement::Below("minecraft:chat".into()) });
    }

    /// Client: K opens the screen (build stage: `mod_screen_open`, drawn in `screen_render_post`).
    fn client_tick_pre(&self, host: &mut dyn ClientHost, _: &mut ClientTickPreEvent) {
        while host.consume_click(MOD_ID, SCREEN_KEY) {
            let _ = host.mod_screen_open(MOD_ID);
        }
    }

    /// Client: the level-up toast (build stage).
    fn gui_layer_draw(&self, _host: &dyn ClientHost, _event: &mut GuiLayerDrawEvent<'_>) {}

    // Build stage, server side, each a thin call into the pure modules above:
    // - add_server_reload_listeners: `FileToIdConverter::json("skills")` → `tree::load`, log each
    //   `Problem`, publish, resend `TreesSync` and reconcile every player's perks.
    // - player_load_from_file / player_save_to_file: `store::read` / `store::write`.
    // - player_logged_in, player_respawned: send trees and progress, reconcile perks.
    // - player_stat_award, player_destroyed_block: `sources::gains` → `share::split` →
    //   `Progress::earn` for each player → send progress.
}
