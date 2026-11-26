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
// Kit (ameliiom/kit), the shared code: the installed Kit mod, or its source in place for
// local builds (`kit_local`, horizon-mods/.cargo/config.toml).
#[cfg(kit_local)]
#[allow(dead_code)]
#[path = "../../kit/src/lib.rs"]
mod kit;
#[cfg(not(kit_local))]
use ::kit;
mod client;
mod look;
mod server;
pub mod payloads;
pub mod perks;
pub mod progress;
pub mod share;
pub mod sources;
mod store;
pub mod tree;

use mod_api::{
    AddPackFindersEvent, AddServerReloadListenersEvent, ClientHost, EntityPlaceEvent, HostSide,
    Identifier, PackPosition, PackRegistration, PackType, Text, ClientTickPreEvent, Flow, GuiLayer,
    GuiLayerDrawEvent, GuiLayerPlacement, Hooks, Host, IPayloadContext, KeyRegistration,
    PayloadId, PlayerDestroyedBlockEvent, PlayerLoadFromFileEvent, PlayerLoggedInEvent,
    PlayerRespawnedEvent, PlayerSaveToFileEvent, PlayerStatAwardEvent, RegisterGuiLayersEvent,
    Registrar, RegistrationContext, ScreenMouseButtonPressedPreEvent, ScreenMouseDraggedPreEvent,
    ScreenRenderPostEvent, ServerHost, ServerTickEvent, ServerTickPhase,
    ClientPlayerLoggingOutEvent,
};
#[allow(unused_imports)]
use mod_api::prelude::*;
use std::sync::Mutex;

pub(crate) const MOD_ID: &str = "skills";
const SCREEN_KEY: &str = "open";
/// The HUD layer the level-up toast draws in.
const LAYER: &str = "skills:toast";

/// Settings (Options > Mods > Skills).
pub(crate) mod setting {
    use crate::kit::setting::{Number, Toggle};
    pub const SHARE_RADIUS: Number = Number { key: "share_radius", min: 0.0, max: 128.0, step: 1.0, default: 32.0 };
    pub const SHARE_FRACTION: Number = Number { key: "share_fraction", min: 0.0, max: 1.0, step: 0.05, default: 0.5 };
    pub const TOASTS: Toggle = Toggle::on("toasts");
}

/// What the client knows: the server's trees and this player's progress, as last sent.
#[derive(Default)]
pub(crate) struct Seen {
    pub trees: tree::Trees,
    pub progress: progress::Progress,
    /// Whether the server has Skills (it sent trees).
    pub served: bool,
    /// Whether progress has arrived since joining.
    pub received: bool,
}

pub(crate) static SEEN: Mutex<Option<Seen>> = Mutex::new(None);

/// On a client: whether this player has unlocked a node that gives the tag `tag`, as the server
/// last told. Other client-side mods read their comforts here (the tag itself lives on the
/// server's player only). False before progress arrives or when the server has no Skills.
pub fn has_tag(tag: &str) -> bool {
    lock(&SEEN).as_ref().is_some_and(|s| s.served && perks::tags(&s.trees, &s.progress).0.contains(tag))
}

pub(crate) use kit::sync::lock;

/// Client: the server's trees arrived.
fn on_trees(sync: payloads::TreesSync, _: &mut dyn IPayloadContext) -> Result<(), String> {
    let trees = sync.json.read()?;
    let mut seen = lock(&SEEN);
    let seen = seen.get_or_insert_with(Seen::default);
    seen.trees = trees;
    seen.served = true;
    Ok(())
}

/// Client: this player's progress arrived (build stage: a level gained queues a toast).
fn on_progress(sync: payloads::ProgressSync, _: &mut dyn IPayloadContext) -> Result<(), String> {
    let progress: progress::Progress = sync.json.read()?;
    let mut seen = lock(&SEEN);
    let seen = seen.get_or_insert_with(Seen::default);
    // The first progress after joining shows where you are; only later ones are level-ups.
    if seen.received {
        client::progressed(&seen.trees, &seen.progress, &progress);
    }
    seen.progress = progress;
    seen.received = true;
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
        for spec in [setting::SHARE_RADIUS.spec(MOD_ID), setting::SHARE_FRACTION.spec(MOD_ID), setting::TOASTS.spec(MOD_ID)] {
            let _ = r.register_setting(spec);
        }
        let mut handlers = r.payload_handlers();
        let mut view = handlers.registrar("1").optional();
        let _ = payloads::TREES.set(view.play_to_client_typed(PayloadId("skills:trees".into()), on_trees));
        let _ = payloads::PROGRESS.set(view.play_to_client_typed(PayloadId("skills:progress".into()), on_progress));
        let _ = payloads::UNLOCK.set(view.play_to_server_typed(PayloadId("skills:unlock".into()), server::unlock));
    }

    fn register_gui_layers(&self, _: &RegistrationContext, event: &mut RegisterGuiLayersEvent<'_>) {
        event.layers.push(GuiLayer { id: LAYER.into(), placement: GuiLayerPlacement::Below("minecraft:chat".into()) });
    }

    /// Client: K opens the screen (build stage: `mod_screen_open`, drawn in `screen_render_post`).
    fn client_tick_pre(&self, host: &mut dyn ClientHost, _: &mut ClientTickPreEvent) {
        while host.consume_click(MOD_ID, SCREEN_KEY) {
            client::open(host);
        }
        client::tick(host);
    }

    fn client_player_logging_out(&self, _: &mut dyn ClientHost, _: &mut ClientPlayerLoggingOutEvent) {
        *lock(&SEEN) = None;
    }

    fn gui_layer_draw(&self, host: &dyn ClientHost, event: &mut GuiLayerDrawEvent<'_>) {
        if event.layer == LAYER {
            client::draw_toasts(host, event.draw);
        }
    }

    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        client::draw_screen(host, event.draw, event.screen.mouse);
    }

    fn screen_mouse_button_pressed_pre(&self, host: &dyn ClientHost, event: &mut ScreenMouseButtonPressedPreEvent<'_>) -> Flow {
        if client::press(host, event.button, event.screen.mouse) { Flow::Cancel } else { Flow::Continue }
    }

    fn screen_mouse_dragged_pre(&self, host: &dyn ClientHost, event: &mut ScreenMouseDraggedPreEvent<'_>) -> Flow {
        if client::drag(host, event.button, event.delta) { Flow::Cancel } else { Flow::Continue }
    }

    /// The default trees, as an always-on data pack bundled in the mod, below every other pack so a
    /// world's or another mod's pack overrides or extends them.
    fn add_pack_finders(&self, _: HostSide, event: &mut AddPackFindersEvent<'_>) {
        let Ok(location) = Identifier::parse("skills:assets/skills/packs/defaults") else { return };
        let mut pack = PackRegistration::new(location, PackType::ServerData, Text::translate("skills.pack.defaults", vec![]));
        pack.always_active = true;
        pack.position = PackPosition::Bottom;
        pack.hidden = true;
        if let Err(e) = event.packs.add_pack_finders(pack) {
            eprintln!("[skills] default trees pack: {e:?}");
        }
    }

    fn add_server_reload_listeners(&self, _: &dyn Host, event: &mut AddServerReloadListenersEvent<'_>) {
        server::add_listener(event.listeners);
    }

    fn server_tick(&self, host: &mut dyn ServerHost, event: &mut ServerTickEvent<'_>) {
        if matches!(event.phase, ServerTickPhase::ServerTickEnd { .. }) {
            server::tick(host);
        }
    }

    fn player_load_from_file(&self, _: &mut dyn ServerHost, event: &mut PlayerLoadFromFileEvent<'_>) {
        server::load(event.player, event.directory);
    }

    fn player_save_to_file(&self, _: &mut dyn ServerHost, event: &mut PlayerSaveToFileEvent<'_>) {
        server::save(event.player, event.directory);
    }

    fn player_logged_in(&self, _: &mut dyn ServerHost, event: &mut PlayerLoggedInEvent<'_>) {
        server::joined(event.player);
    }

    fn player_respawned(&self, _: &mut dyn ServerHost, event: &mut PlayerRespawnedEvent<'_>) {
        server::respawned(event.player);
    }

    fn player_stat_award(&self, host: &mut dyn ServerHost, event: &mut PlayerStatAwardEvent<'_>) -> Flow {
        let (stat_type, stat) = (event.stat_type.clone(), event.stat.clone());
        server::stat(host, event.player, &stat_type, &stat, event.value);
        Flow::Continue
    }

    fn player_destroyed_block(&self, host: &mut dyn ServerHost, event: &mut PlayerDestroyedBlockEvent) {
        server::block(host, tree::BlockEvent::Broken, event.state, None);
    }

    /// A player placing a block (a mob's placement has no player and earns nothing).
    fn entity_place(&self, host: &mut dyn ServerHost, event: &mut EntityPlaceEvent) -> Flow {
        if let (Some(placer), false) = (event.placer, event.cancelled) {
            server::block(host, tree::BlockEvent::Placed, event.state, Some(placer));
        }
        Flow::Continue
    }
}
