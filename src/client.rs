//! The client: the skills screen (key `K`) and the level-up toast, drawn from the game's own
//! sprites and text. Geometry comes from `look.rs`; what to show comes from `progress.rs`.
use crate::look::{self, sprite, Rect};
use crate::payloads::{self, Unlock};
use crate::progress::{NodeState, Progress, View};
use crate::tree::{Node, Perk, Tree, Trees};
use crate::{lock, rtl, Seen, MOD_ID, SEEN};
use mod_api::client::hud_layout::Blend;
use mod_api::text::TextStyle;
use mod_api::{
    ClientHost, DrawContext, GuiDraw, Identifier, SoundPlay, Text,
};
#[allow(unused_imports)]
use mod_api::prelude::*;
use std::sync::Mutex;

/// How long a toast shows, and how long it takes to slide in and out, in client ticks.
const TOAST_TICKS: u32 = 100;
const TOAST_SLIDE: f32 = 6.0;
const TOAST: [f32; 2] = [160.0, 32.0];
/// Vanilla's title colour inside the advancements window, and the experience number's green.
const TITLE: u32 = 0xFF40_4040;
const XP_GREEN: u32 = 0xFF80_FF20;

struct Toast {
    tree: String,
    level: u32,
    /// Levels gained at once, each a point.
    gained: u32,
    at: u32,
}

#[derive(Default)]
struct Ui {
    tab: usize,
    /// Each tree's pan, once dragged (centred until then).
    pans: Vec<(String, [f32; 2])>,
    toasts: Vec<Toast>,
    ticks: u32,
    /// Where a press started, to tell a click from a drag.
    pressed: Option<[f32; 2]>,
    dragged: bool,
    /// The screen size at the last frame, for input hooks that don't carry one.
    size: [f32; 2],
    /// A click on an open node, sent on the next tick (sending needs the mutable host).
    unlock: Option<Unlock>,
}

static UI: Mutex<Option<Ui>> = Mutex::new(None);

fn ui<R>(f: impl FnOnce(&mut Ui) -> R) -> R {
    f(lock(&UI).get_or_insert_with(Ui::default))
}

pub fn tick(host: &mut dyn ClientHost) {
    let unlock = ui(|u| {
        u.ticks = u.ticks.wrapping_add(1);
        let now = u.ticks;
        u.toasts.retain(|t| now.wrapping_sub(t.at) < TOAST_TICKS);
        u.unlock.take()
    });
    if let (Some(request), Some(token)) = (unlock, payloads::UNLOCK.get()) {
        let _ = token.send(host, &request);
    }
}

/// New progress arrived after an earlier one: queue a toast for each tree that went up a level.
pub fn progressed(trees: &Trees, before: &Progress, after: &Progress) {
    ui(|u| {
        for tree in &trees.0 {
            let (a, b) = (View::new(tree, before).level().level, View::new(tree, after).level().level);
            if b > a {
                let at = u.ticks;
                u.toasts.push(Toast { tree: tree.id.clone(), level: b, gained: b - a, at });
            }
        }
    });
}

fn tr(host: &dyn ClientHost, key: &str, fallback: &str) -> String {
    host.translate(key).unwrap_or_else(|| fallback.to_owned())
}

/// One of the game's GUI sprites as vanilla's `blitSprite` draws it, tinted; `region` is
/// `[sprite w, sprite h, u, v]` to draw only part of it.
fn blit(draw: &mut dyn DrawContext, name: &str, [x, y, w, h]: Rect, tint: u32, region: Option<[i32; 4]>) {
    let op = GuiDraw::Sprite {
        sprite: name.to_owned().into(),
        x: x.round() as i32,
        y: y.round() as i32,
        w: w.round() as i32,
        h: h.round() as i32,
        region,
        color: tint,
        blend: Blend::Textured,
    };
    let _ = draw.gui_paint(&[op]);
}

fn icon(host: &dyn ClientHost, draw: &mut dyn DrawContext, id: &str, at: [f32; 2]) {
    if let Some(key) = Identifier::parse(id).ok().and_then(|i| host.item_key(&i)) {
        draw.item(key, at, 16.0);
    }
}

fn node_name(host: &dyn ClientHost, node: &Node) -> String {
    node.title.as_deref().and_then(|k| host.translate(k)).unwrap_or_else(|| node.id.clone())
}

fn tree_name(host: &dyn ClientHost, tree: &Tree) -> String {
    tr(host, &tree.title, &tree.id)
}

// ---- Screen ----

/// A tab's icon: its tree's, or an arrow for the next page.
fn tab_icon(seen: &Seen, tab: look::Tab) -> &str {
    match tab {
        look::Tab::Tree(i) => &seen.trees.0[i].icon,
        look::Tab::More(_) => "minecraft:spectral_arrow",
    }
}

fn current(seen: &Seen, tab: usize) -> Option<&Tree> {
    seen.trees.0.get(tab.min(seen.trees.0.len().saturating_sub(1)))
}

fn pan_of(u: &Ui, tree: &Tree, inside: Rect) -> [f32; 2] {
    u.pans
        .iter()
        .find(|(id, _)| *id == tree.id)
        .map(|(_, p)| *p)
        .unwrap_or_else(|| look::centre_pan(inside, look::bounds(tree.nodes.iter().map(|n| n.at))))
}

/// The window texture drawn nine-slice by its frame, so it can fill the screen.
fn window_frame(draw: &mut dyn DrawContext, [x, y, w, h]: Rect) {
    // The window is the top-left 252×140 of a 256×256 texture.
    let (tex, sheet) = ([252.0, 140.0], [256, 256]);
    let [l, t, r, b] = look::BORDER;
    let cols = [(0.0, l, x, l), (l, tex[0] - l - r, x + l, w - l - r), (tex[0] - r, r, x + w - r, r)];
    let rows = [(0.0, t, y, t), (t, tex[1] - t - b, y + t, h - t - b), (tex[1] - b, b, y + h - b, b)];
    let mut ops = Vec::new();
    for (ri, (v, sh, dy, dh)) in rows.iter().enumerate() {
        for (ci, (u, sw, dx, dw)) in cols.iter().enumerate() {
            if ri == 1 && ci == 1 {
                continue; // the middle is the tree, drawn underneath
            }
            ops.push(GuiDraw::Blit {
                texture: sprite::WINDOW.into(),
                blend: Blend::Textured,
                x: dx.round() as i32,
                y: dy.round() as i32,
                w: dw.round() as i32,
                h: dh.round() as i32,
                uv: [*u, *v],
                src: [*sw as i32, *sh as i32],
                tex: sheet,
                color: 0xFFFF_FFFF,
            });
        }
    }
    let _ = draw.gui_paint(&ops);
}

/// The tree's background: its block texture in 16-pixel tiles, moving with the pan.
fn background(draw: &mut dyn DrawContext, texture: &str, inside: Rect, pan: [f32; 2]) {
    let [ix, iy, iw, ih] = inside;
    let ox = pan[0].rem_euclid(16.0) - 16.0;
    let oy = pan[1].rem_euclid(16.0) - 16.0;
    // `minecraft:block/stone` → the blit's texture path under `textures/`.
    let texture = texture.split_once(':').map_or(texture, |(_, path)| path).to_owned();
    let mut ops = Vec::new();
    let mut ty = iy + oy;
    while ty < iy + ih {
        let mut tx = ix + ox;
        while tx < ix + iw {
            ops.push(GuiDraw::Blit {
                texture: texture.clone().into(),
                blend: Blend::Textured,
                x: tx.round() as i32,
                y: ty.round() as i32,
                w: 16,
                h: 16,
                uv: [0.0, 0.0],
                src: [16, 16],
                tex: [16, 16],
                color: 0xFFFF_FFFF,
            });
            tx += 16.0;
        }
        ty += 16.0;
    }
    let _ = draw.gui_paint(&ops);
}

/// The connections: a black line under a white one, as the advancement tree draws them.
fn links(draw: &mut dyn DrawContext, tree: &Tree, rect: &dyn Fn(&Node) -> Rect) {
    for (width, colour) in [(3.0, 0xFF00_0000), (1.0, 0xFFFF_FFFF)] {
        for child in &tree.nodes {
            for parent in child.needs.iter().filter_map(|id| tree.node(id)) {
                let pts = look::link(rect(parent), rect(child));
                for seg in pts.windows(2) {
                    let (a, b) = (seg[0], seg[1]);
                    let half = width / 2.0;
                    let r = [a[0].min(b[0]) - half, a[1].min(b[1]) - half, (a[0] - b[0]).abs() + width, (a[1] - b[1]).abs() + width];
                    draw.fill(r, colour);
                }
            }
        }
    }
}

fn perk_lines(host: &dyn ClientHost, node: &Node) -> Vec<Text> {
    node.perks
        .iter()
        .map(|perk| {
            let (attribute, amount, operation) = match perk {
                Perk::Attribute { attribute, amount, operation } => (attribute, amount, operation),
                Perk::Tag { tag } => return coloured(tr(host, &format!("skills.perk.{tag}"), tag), "blue"),
            };
            let (key, shown, blue) = look::modifier_text(*amount, *operation);
            let (ns, path) = attribute.split_once(':').unwrap_or(("minecraft", attribute));
            let name_key = [format!("attribute.name.{path}"), format!("attribute.name.{ns}.{path}")]
                .into_iter()
                .find(|k| host.translate(k).is_some())
                .unwrap_or_else(|| format!("attribute.name.{path}"));
            let mut style = TextStyle::default();
            style.color = Some(if blue { "blue" } else { "red" }.into());
            Text::translate(key, vec![Text::literal(shown), Text::translate(name_key, vec![])]).with_style(style)
        })
        .collect()
}

fn coloured(text: String, colour: &str) -> Text {
    let mut style = TextStyle::default();
    style.color = Some(colour.into());
    Text::literal(text).with_style(style)
}

pub fn open(host: &mut dyn ClientHost) {
    let _ = host.mod_screen_open(MOD_ID);
}

fn mine(host: &dyn ClientHost) -> bool {
    host.mod_screen() == Some(MOD_ID)
}

pub fn draw_screen(host: &dyn ClientHost, draw: &mut dyn DrawContext, mouse: [f32; 2]) {
    if !mine(host) {
        return;
    }
    let size = draw.size();
    ui(|u| u.size = size);
    // The game's own screen backdrop: the world blurred and dimmed behind the window.
    host.apply_screen_blur();
    draw.draw_menu_background([0.0, 0.0, size[0], size[1]], true);
    let rtl = rtl::right_to_left(host);
    let window = look::window(size);
    let inside = look::inside(window);
    let seen_guard = lock(&SEEN);
    let empty = Seen::default();
    let seen = seen_guard.as_ref().unwrap_or(&empty);
    let Some(tree) = current(seen, ui(|u| u.tab)) else {
        window_frame(draw, window);
        draw.fill(inside, 0xFF00_0000);
        let text = tr(host, if seen.served { "skills.empty" } else { "skills.not_served" }, "Skills");
        let w = draw.text_width(&text);
        draw.text(&text, [inside[0] + (inside[2] - w) / 2.0, inside[1] + inside[3] / 2.0 - 4.0], 0xFFFF_FFFF, true);
        return;
    };
    let tab = seen.trees.0.iter().position(|t| t.id == tree.id).unwrap_or(0);
    let pan = ui(|u| pan_of(u, tree, inside));
    let view = View::new(tree, &seen.progress);
    let rect = |n: &Node| look::node(inside, n.at, pan, rtl);

    // Tabs behind the window, then the tree, then the window, then the selected tab over it.
    let tabs = look::tabs(window, seen.trees.0.len(), tab, rtl);
    for (slot, (t, r)) in tabs.iter().enumerate() {
        if *t == look::Tab::Tree(tab) {
            continue;
        }
        blit(draw, &sprite::tab(slot == 0, false, rtl), *r, 0xFFFF_FFFF, None);
        icon(host, draw, tab_icon(seen, *t), [r[0] + 6.0, r[1] + 9.0]);
    }
    if draw.push_clip(inside).is_ok() {
        background(draw, &tree.background, inside, pan);
        links(draw, tree, &rect);
        for n in &tree.nodes {
            let r = rect(n);
            let state = view.state(&n.id);
            let frame = if state == NodeState::Unlocked { sprite::FRAME_UNLOCKED } else { sprite::FRAME };
            let tint = if state == NodeState::Locked { 0xFF60_6060 } else { 0xFFFF_FFFF };
            blit(draw, frame, r, tint, None);
            icon(host, draw, &n.icon, [r[0] + 5.0, r[1] + 5.0]);
            if state == NodeState::Locked {
                draw.fill([r[0] + 5.0, r[1] + 5.0, 16.0, 16.0], 0x8000_0000);
            }
        }
        let _ = draw.pop_clip();
    }
    window_frame(draw, window);
    if let Some((slot, (_, r))) = tabs.iter().enumerate().find(|(_, (t, _))| *t == look::Tab::Tree(tab)) {
        blit(draw, &sprite::tab(slot == 0, true, rtl), *r, 0xFFFF_FFFF, None);
        icon(host, draw, &tree.icon, [r[0] + 6.0, r[1] + 9.0]);
    }

    // Header: the tree's name at the reading start; level, bar and points at the end.
    let level = view.level();
    let title = tree_name(host, tree);
    let head = look::header(window, rtl);
    let tw = draw.text_width(&title);
    draw.text(&title, [if rtl { head.title_end - tw } else { head.title_start }, head.y], TITLE, false);
    let points = tr(host, "skills.points", "Points: %s").replace("%s", &view.points().to_string());
    let lv = level.level.to_string();
    let parts = look::header_end(head, draw.text_width(&lv), draw.text_width(&points), rtl);
    // The HUD's experience bar at its own size, filling from the reading start.
    blit(draw, sprite::XP_BACK, parts.bar, 0xFFFF_FFFF, None);
    let fill = if level.next > 0.0 { (level.into / level.next).clamp(0.0, 1.0) as f32 } else { 1.0 };
    let w = (look::BAR * fill).round();
    if w > 0.0 {
        let u = if rtl { look::BAR - w } else { 0.0 };
        let at = [parts.bar[0] + u, parts.bar[1], w, parts.bar[3]];
        blit(draw, sprite::XP_FILL, at, 0xFFFF_FFFF, Some([look::BAR as i32, 5, u as i32, 0]));
    }
    for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
        draw.text(&lv, [parts.level[0] + dx, parts.level[1] + dy], 0xFF00_0000, false);
    }
    draw.text(&lv, parts.level, XP_GREEN, false);
    draw.text(&points, parts.points, TITLE, false);

    // Hover: a tab's name, or a node's tooltip.
    if let Some((t, _)) = tabs.iter().find(|(_, r)| look::contains(*r, mouse)) {
        let name = match t {
            look::Tab::Tree(i) => tree_name(host, &seen.trees.0[*i]),
            look::Tab::More(_) => tr(host, "skills.more", "More skills"),
        };
        let _ = draw.gui_tooltip(&[Text::literal(name)], mouse);
        return;
    }
    if !look::contains(inside, mouse) {
        return;
    }
    let Some(n) = tree.nodes.iter().find(|n| look::contains(rect(n), mouse)) else { return };
    let state = view.state(&n.id);
    let mut lines = vec![coloured(node_name(host, n), if state == NodeState::Unlocked { "gold" } else { "white" })];
    lines.extend(perk_lines(host, n));
    let cost = tr(host, "skills.cost", "Cost: %s").replace("%s", &n.cost.to_string());
    lines.push(coloured(cost, "gray"));
    let (key, colour) = match state {
        NodeState::Unlocked => ("skills.state.unlocked", "gold"),
        NodeState::Open => ("skills.state.open", "green"),
        NodeState::Affordable => ("skills.state.points", "red"),
        NodeState::Locked => ("skills.state.locked", "red"),
    };
    lines.push(coloured(tr(host, key, key), colour));
    let _ = draw.gui_tooltip(&lines, mouse);
}

// ---- Input ----

/// A press: remembered, so a release without a drag is a click.
pub fn press(host: &dyn ClientHost, button: i32, mouse: [f32; 2]) -> bool {
    if !mine(host) || button != 0 {
        return false;
    }
    ui(|u| {
        u.pressed = Some(mouse);
        u.dragged = false;
    });
    click(host, mouse);
    true
}

fn click(host: &dyn ClientHost, mouse: [f32; 2]) {
    let size = host_size(host);
    let rtl = rtl::right_to_left(host);
    let window = look::window(size);
    let inside = look::inside(window);
    let seen_guard = lock(&SEEN);
    let Some(seen) = seen_guard.as_ref() else { return };
    let selected = ui(|u| u.tab).min(seen.trees.0.len().saturating_sub(1));
    let tabs = look::tabs(window, seen.trees.0.len(), selected, rtl);
    if let Some((t, _)) = tabs.iter().find(|(_, r)| look::contains(*r, mouse)) {
        let (look::Tab::Tree(i) | look::Tab::More(i)) = *t;
        ui(|u| u.tab = i);
        let _ = host.play_local_sound(&SoundPlay::ui("minecraft:ui.button.click"), 0);
        return;
    }
    let Some(tree) = current(seen, ui(|u| u.tab)) else { return };
    if !look::contains(inside, mouse) {
        return;
    }
    let pan = ui(|u| pan_of(u, tree, inside));
    let Some(n) = tree.nodes.iter().find(|n| look::contains(look::node(inside, n.at, pan, rtl), mouse)) else {
        return;
    };
    if View::new(tree, &seen.progress).state(&n.id) != NodeState::Open {
        return;
    }
    ui(|u| u.unlock = Some(Unlock { tree: tree.id.clone(), node: n.id.clone() }));
    let _ = host.play_local_sound(&SoundPlay::ui("minecraft:ui.toast.challenge_complete"), 0);
}

fn host_size(_: &dyn ClientHost) -> [f32; 2] {
    ui(|u| u.size)
}

/// A drag pans the tree (Arabic pans mirrored, as the tree is drawn).
pub fn drag(host: &dyn ClientHost, button: i32, delta: [f32; 2]) -> bool {
    if !mine(host) || button != 0 {
        return false;
    }
    let rtl = rtl::right_to_left(host);
    let inside = look::inside(look::window(host_size(host)));
    let seen_guard = lock(&SEEN);
    let Some(tree) = seen_guard.as_ref().and_then(|s| current(s, ui(|u| u.tab))) else { return true };
    ui(|u| {
        let now = pan_of(u, tree, inside);
        let dx = if rtl { -delta[0] } else { delta[0] };
        let next = [now[0] + dx, now[1] + delta[1]];
        u.pans.retain(|(id, _)| *id != tree.id);
        u.pans.push((tree.id.clone(), next));
        u.dragged = true;
    });
    true
}

// ---- Toast ----

pub fn draw_toasts(host: &dyn ClientHost, draw: &mut dyn DrawContext) {
    if !host.setting(MOD_ID, crate::setting::TOASTS).is_none_or(|v| v.trim() != "false") {
        return;
    }
    // The game's toast corner (top right in every language, as vanilla's toasts); Arabic mirrors
    // the toast's inside: icon at the right, text ending beside it.
    let [w, _] = draw.size();
    let rtl = rtl::right_to_left(host);
    let seen_guard = lock(&SEEN);
    let Some(seen) = seen_guard.as_ref() else { return };
    let (now, toasts): (u32, Vec<(String, u32, u32, u32)>) =
        ui(|u| (u.ticks, u.toasts.iter().map(|t| (t.tree.clone(), t.level, t.gained, t.at)).collect()));
    for (i, (tree_id, level, gained, at)) in toasts.iter().enumerate() {
        let Some(tree) = seen.trees.get(tree_id) else { continue };
        let age = now.wrapping_sub(*at) as f32;
        let shown = look::toast_slide(age, TOAST_TICKS as f32, TOAST_SLIDE);
        let x = w - TOAST[0] * shown;
        let y = i as f32 * TOAST[1];
        blit(draw, sprite::TOAST, [x, y, TOAST[0], TOAST[1]], 0xFFFF_FFFF, None);
        let title = tr(host, "skills.toast.title", "%1$s %2$s")
            .replace("%1$s", &tree_name(host, tree))
            .replace("%2$s", &level.to_string());
        let key = if *gained == 1 { "skills.toast.point" } else { "skills.toast.points" };
        let points = tr(host, key, "+%s").replace("%s", &gained.to_string());
        let (icon_x, text_end) = if rtl { (x + TOAST[0] - 24.0, x + TOAST[0] - 30.0) } else { (x + 8.0, 0.0) };
        icon(host, draw, &tree.icon, [icon_x, y + 8.0]);
        for (text, ty, colour) in [(title, 7.0, 0xFFFF_FF00), (points, 18.0, 0xFFFF_FFFF)] {
            let tx = if rtl { text_end - draw.text_width(&text) } else { x + 30.0 };
            draw.text(&text, [tx, y + ty], colour, false);
        }
    }
}
