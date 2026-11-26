//! The server: loads the trees, keeps each player's progress, turns actions into experience,
//! checks unlocks and reconciles perks. Every rule lives in the pure modules; this only reads the
//! game, calls them and writes back.
use crate::payloads::{self, ProgressSync, TreesSync, Unlock};
use crate::progress::Progress;
use crate::share::{self, Near};
use crate::sources::{self, Action, Tags};
use crate::tree::{self, BlockEvent, Operation, Trees};
use crate::{lock, perks, store};
use mod_api::{
    BlockMetadata, HostBlockStates, AttributeModifier, AttributeOperation, FileToIdConverter, Host, IPayloadContext, Identifier,
    ModifierLifetime, PlayerRef, ReloadApply, ReloadResourceError, ServerHost,
    ServerReloadContext, ServerReloadListener,
};
#[allow(unused_imports)]
use mod_api::prelude::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Ticks a player's trees are retried for (10 seconds).
const RETRIES: u32 = 200;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct State {
    trees: Trees,
    /// Trees a reload parsed, waiting for the next tick to be checked against the registries.
    pending: Option<Trees>,
    players: HashMap<u128, Progress>,
    /// Players whose progress changed since it was last sent.
    dirty: BTreeSet<u128>,
    /// Players who still need the trees (just joined, or a reload changed them): retried each
    /// tick until the client takes them, as a join's first ticks can't carry payloads yet; a
    /// client without Skills never takes them, so each gets `RETRIES` ticks.
    unsynced: BTreeMap<u128, u32>,
    /// Players whose perks are reconciled on the next tick: a join or respawn hook runs before
    /// the new player entity takes modifiers.
    unsettled: BTreeSet<u128>,
    /// The world's `players/data`, from the first load or save.
    directory: Option<PathBuf>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    f(lock(&STATE).get_or_insert_with(State::default))
}

/// The registry a statistic type's entries belong to, for tags. Vanilla's stat types (a platform
/// constant: mods rarely add one); any other uses `minecraft:custom_stat`.
fn stat_registry(stat_type: &str) -> &'static str {
    match stat_type {
        "minecraft:mined" => "minecraft:block",
        "minecraft:killed" | "minecraft:killed_by" => "minecraft:entity_type",
        "minecraft:crafted" | "minecraft:used" | "minecraft:broken" | "minecraft:picked_up"
        | "minecraft:dropped" => "minecraft:item",
        _ => "minecraft:custom_stat",
    }
}

struct RegistryTags<'a> {
    host: &'a dyn Host,
    registry: &'a str,
}

impl Tags for RegistryTags<'_> {
    fn has(&self, id: &str, tag: &str) -> bool {
        self.host
            .registry_entry(self.registry, id)
            .and_then(|entry| self.host.registry_entry_in_tag(&entry, tag))
            .unwrap_or(false)
    }
}

// ---- Data ----

struct Listener;

impl ServerReloadListener for Listener {
    fn prepare(&self, context: &ServerReloadContext) -> Result<Box<dyn ReloadApply>, ReloadResourceError> {
        let files: Vec<(String, Vec<u8>)> = context
            .list()
            .filter_map(|id| Some((id.to_string(), context.read(id)?.to_vec())))
            .collect();
        // Attributes are checked on the next tick, where the registries can be read.
        let (trees, problems) = tree::load(files.iter().map(|(id, b)| (id.clone(), b.as_slice())), |_| true);
        for p in problems {
            eprintln!("[skills] {}: {}", p.file, p.what);
        }
        Ok(<dyn ReloadApply>::new(move || with(|s| s.pending = Some(trees))))
    }
}

pub fn add_listener(listeners: &mut dyn mod_api::ServerReloadListeners) {
    let Ok(converter) = FileToIdConverter::json("skills") else { return };
    let Ok(id) = Identifier::parse("skills:trees") else { return };
    if let Err(e) = listeners.add_listener(id, converter, Arc::new(Listener)) {
        eprintln!("[skills] reload listener: {e:?}");
    }
}

/// Each tick: publish trees a reload left (checked now), send changed progress.
pub fn tick(host: &mut dyn ServerHost) {
    if let Some(trees) = with(|s| s.pending.take()) {
        let trees = check_attributes(&*host, trees);
        with(|s| s.trees = trees);
        for player in host.players().server_players() {
            with(|s| {
                s.unsynced.insert(player.uuid(), 0);
                s.unsettled.insert(player.uuid());
            });
        }
    }
    let unsettled = with(|s| std::mem::take(&mut s.unsettled));
    for uuid in unsettled {
        if let Some(player) = host.player(uuid) {
            reconcile(host, player);
        }
    }
    let unsynced: Vec<u128> = with(|s| s.unsynced.keys().copied().collect());
    for uuid in unsynced {
        let sent = host.player(uuid).map(|player| send_trees(host, player));
        with(|s| {
            let tries = s.unsynced.entry(uuid).or_default();
            *tries += 1;
            if sent != Some(false) || *tries >= RETRIES {
                s.unsynced.remove(&uuid);
            }
            if sent == Some(true) {
                s.dirty.insert(uuid);
            }
        });
    }
    let dirty = with(|s| std::mem::take(&mut s.dirty));
    for uuid in dirty {
        if let Some(player) = host.player(uuid)
            && !send_progress(host, player)
        {
            with(|s| s.dirty.insert(uuid));
        }
    }
}

/// Drops nodes whose perks name an attribute the game doesn't register.
fn check_attributes(host: &dyn Host, trees: Trees) -> Trees {
    let mut trees = trees;
    for t in &mut trees.0 {
        t.nodes.retain(|n| {
            n.perks.iter().all(|perk| {
                let tree::Perk::Attribute { attribute, .. } = perk else { return true };
                let known = host.registry_entry("minecraft:attribute", attribute).is_some();
                if !known {
                    eprintln!("[skills] {}: node {}: unknown attribute {attribute}", t.id, n.id);
                }
                known
            })
        });
        let ids: BTreeSet<String> = t.nodes.iter().map(|n| n.id.clone()).collect();
        t.nodes.retain(|n| n.needs.iter().all(|d| ids.contains(d)));
    }
    trees
}

// ---- Players ----

pub fn load(uuid: u128, directory: &std::path::Path) {
    let progress = store::read(directory, &uuid_text(uuid));
    with(|s| {
        s.directory.get_or_insert_with(|| directory.to_path_buf());
        s.players.insert(uuid, progress);
    });
}

pub fn save(uuid: u128, directory: &std::path::Path) {
    let Some(progress) = with(|s| s.players.get(&uuid).cloned()) else { return };
    if let Err(e) = store::write(directory, &uuid_text(uuid), &progress) {
        eprintln!("[skills] saving {}: {e}", uuid_text(uuid));
    }
}

/// The game's own UUID spelling (`8-4-4-4-12` hex), as the player's `.dat` file is named.
fn uuid_text(uuid: u128) -> String {
    let h = format!("{uuid:032x}");
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

pub fn joined(uuid: u128) {
    with(|s| {
        s.unsynced.insert(uuid, 0);
        s.unsettled.insert(uuid);
    });
}

pub fn respawned(uuid: u128) {
    with(|s| s.unsettled.insert(uuid));
}

/// Whether the client took the trees.
fn send_trees(host: &mut dyn ServerHost, player: PlayerRef) -> bool {
    let Some(token) = payloads::TREES.get() else { return true };
    let json = match with(|s| payloads::Json::of(&s.trees)) {
        Ok(json) => json,
        Err(e) => {
            eprintln!("[skills] trees as JSON: {e}");
            return true;
        }
    };
    token.send(host, player, &TreesSync { json }).is_ok()
}

/// Whether the client took its progress.
fn send_progress(host: &mut dyn ServerHost, player: PlayerRef) -> bool {
    let Some(token) = payloads::PROGRESS.get() else { return true };
    let json = match with(|s| payloads::Json::of(&s.players.get(&player.uuid()).cloned().unwrap_or_default())) {
        Ok(json) => json,
        Err(e) => {
            eprintln!("[skills] progress of {} as JSON: {e}", uuid_text(player.uuid()));
            return true;
        }
    };
    token.send(host, player, &ProgressSync { json }).is_ok()
}

// ---- Earning ----

pub fn stat(host: &mut dyn ServerHost, uuid: u128, stat_type: &str, stat: &str, value: i32) {
    if value <= 0 {
        return;
    }
    let gains = with(|s| {
        let tags = RegistryTags { host: &*host, registry: stat_registry(stat_type) };
        sources::gains(&s.trees, &Action::Stat { stat: stat_type, of: stat, amount: f64::from(value) }, &tags)
    });
    earn(host, uuid, gains);
}

/// A block a player broke (`acting player`) or placed (`placer`).
pub fn block(host: &mut dyn ServerHost, on: BlockEvent, state: mod_api::BlockState, placer: Option<mod_api::EntityRef>) {
    let player = match placer {
        Some(entity) => host.players().server_players().into_iter().find(|p| p.entity() == entity),
        None => host.players().acting_player(),
    };
    let Some(player) = player else { return };
    let gains = {
        let read: &dyn Host = &*host;
        let meta = read.block_states();
        let Ok(block) = meta.name(state.block()).map(|n| n.to_owned()) else { return };
        let prop = |k: &str| state.property(&meta, k).ok().flatten().map(str::to_owned);
        let tags = RegistryTags { host: read, registry: "minecraft:block" };
        with(|s| sources::gains(&s.trees, &Action::Block { on, block: &block, state: &prop }, &tags))
    };
    earn(host, player.uuid(), gains);
}

/// Gives `gains` to the earner and each friend's share, marking them to be sent.
fn earn(host: &mut dyn ServerHost, uuid: u128, gains: Vec<(String, f64)>) {
    if gains.is_empty() {
        return;
    }
    let rules = share::Rules {
        radius: crate::setting::SHARE_RADIUS.read(&*host, crate::MOD_ID),
        fraction: crate::setting::SHARE_FRACTION.read(&*host, crate::MOD_ID),
    };
    let everyone = host.players().server_players();
    let read: &dyn Host = &*host;
    let players: Vec<(u128, String, [f64; 3])> = everyone
        .into_iter()
        .filter_map(|p| {
            let dim = p.dimension(read)?.to_string();
            Some((p.uuid(), dim, p.entity().position(read)?))
        })
        .collect();
    let near: Vec<Near<'_, u128>> =
        players.iter().map(|(id, dim, at)| Near { id: *id, dimension: dim, at: *at }).collect();
    let Some(me) = near.iter().position(|n| n.id == uuid) else { return };
    with(|s| {
        for (tree_id, xp) in gains {
            let Some(tree) = s.trees.get(&tree_id).cloned() else { continue };
            for (who, amount) in share::split(&near[me], xp, &near, rules) {
                s.players.entry(who).or_default().earn(&tree, amount);
                s.dirty.insert(who);
            }
        }
    });
}

// ---- Unlocking and perks ----

pub fn unlock(request: Unlock, cx: &mut dyn IPayloadContext) -> Result<(), String> {
    let player = cx.player().map_err(|e| format!("{e:?}"))?;
    let host = cx.server_host().map_err(|e| format!("{e:?}"))?;
    let done = with(|s| {
        let tree = s.trees.get(&request.tree)?.clone();
        let progress = s.players.entry(player.uuid()).or_default();
        progress.unlock(&tree, &request.node).ok()?;
        s.dirty.insert(player.uuid());
        Some(())
    });
    if done.is_some() {
        reconcile(host, player);
    } else {
        // A refused click (stale screen): resend so the client shows the truth.
        with(|s| s.dirty.insert(player.uuid()));
    }
    Ok(())
}

/// Makes the player's `skills:` modifiers and perk tags exactly what their progress earns.
fn reconcile(host: &mut dyn ServerHost, player: PlayerRef) {
    let (wanted, (tags, known)) = with(|s| {
        let progress = s.players.get(&player.uuid()).cloned().unwrap_or_default();
        (perks::wanted(&s.trees, &progress), perks::tags(&s.trees, &progress))
    });
    let has: BTreeSet<String> = player.entity().tags(&*host).unwrap_or_default().into_iter().collect();
    for tag in known.iter().filter(|t| !tags.contains(*t) && has.contains(*t)) {
        player.entity().remove_tag(host, tag);
    }
    for tag in tags.iter().filter(|t| !has.contains(*t)) {
        player.entity().add_tag(host, tag);
    }
    let Some(living) = host.living(player.entity()) else { return };
    let stale: Vec<(String, Identifier)> = {
        let read: &dyn Host = &*host;
        living
            .attributes(read)
            .unwrap_or_default()
            .into_iter()
            .flat_map(|attribute| {
                let attribute = attribute.to_string();
                living
                    .attribute_modifiers(read, &attribute)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|m| perks::ours(&m.id.to_string()))
                    .filter(|m| !wanted.iter().any(|w| w.attribute == attribute && w.id == m.id.to_string()))
                    .map(|m| (attribute.clone(), m.id))
                    .collect::<Vec<_>>()
            })
            .collect()
    };
    for (attribute, id) in stale {
        living.remove_attribute_modifier(host, &attribute, &id);
    }
    for w in wanted {
        let Ok(id) = Identifier::parse(&w.id) else { continue };
        let operation = match w.operation {
            Operation::AddValue => AttributeOperation::AddValue,
            Operation::AddMultipliedBase => AttributeOperation::AddMultipliedBase,
            Operation::AddMultipliedTotal => AttributeOperation::AddMultipliedTotal,
        };
        let modifier = AttributeModifier::new(id, w.amount, operation);
        living.replace_attribute_modifier(host, &w.attribute, modifier, ModifierLifetime::Transient);
    }
}
