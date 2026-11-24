//! The screen's geometry and sprites, in the advancements screen's own measures. Pure: the
//! drawing (`screen.rs`, build stage) only reads these.

/// `[x, y, width, height]` in GUI pixels.
pub type Rect = [f32; 4];

/// The advancements screen's measures (`AdvancementsScreen`, `AdvancementTabType`, `AdvancementWidget`).
pub const BORDER: [f32; 4] = [9.0, 18.0, 9.0, 9.0]; // start, top, end, bottom of the window frame
pub const TAB: [f32; 2] = [28.0, 32.0];
pub const FRAME: f32 = 26.0;
pub const CELL: [f32; 2] = [28.0, 27.0];
/// The gap kept around the window, so the world shows at the screen's edge.
pub const MARGIN: f32 = 16.0;

/// The game's sprites the screen draws with (`textures/gui/sprites/…`), names read from the
/// game's assets.
pub mod sprite {
    pub const FRAME_UNLOCKED: &str = "advancements/task_frame_obtained";
    pub const FRAME: &str = "advancements/task_frame_unobtained";
    /// The hover title boxes (nine-slice, 200×26, border 10).
    pub const BOX_UNLOCKED: &str = "advancements/box_obtained";
    pub const BOX: &str = "advancements/box_unobtained";
    pub const XP_BACK: &str = "hud/experience_bar_background";
    pub const XP_FILL: &str = "hud/experience_bar_progress";
    /// The window frame is a plain texture (252×140), drawn nine-slice by its 9/18/9/9 border.
    pub const WINDOW: &str = "minecraft:textures/gui/advancements/window.png";

    /// The tab sprite as vanilla picks it: the first tab of the row sits on the window's corner
    /// (`left`, or `right` in Arabic, where the row starts at the right), the rest `middle`.
    pub fn tab(first: bool, selected: bool, rtl: bool) -> String {
        let place = match (first, rtl) {
            (false, _) => "middle",
            (true, false) => "left",
            (true, true) => "right",
        };
        let on = if selected { "_selected" } else { "" };
        format!("advancements/tab_above_{place}{on}")
    }
}

/// The window fills the screen but for [`MARGIN`], under the tab row.
pub fn window([w, h]: [f32; 2]) -> Rect {
    let top = MARGIN + TAB[1] - 4.0;
    [MARGIN, top, (w - 2.0 * MARGIN).max(0.0), (h - top - MARGIN).max(0.0)]
}

/// The area inside the frame where the tree is drawn and panned.
pub fn inside(window: Rect) -> Rect {
    let [x, y, w, h] = window;
    [x + BORDER[0], y + BORDER[1], (w - BORDER[0] - BORDER[2]).max(0.0), (h - BORDER[1] - BORDER[3]).max(0.0)]
}

/// Tab `i` of `count` on top of the window, from the reading start (the right in Arabic).
pub fn tab(window: Rect, i: usize, rtl: bool) -> Rect {
    let [x, y, w, _] = window;
    let step = TAB[0] + 4.0;
    let from_start = step * i as f32; // AdvancementTabType.ABOVE: (width + 4) * index
    let tx = if rtl { x + w - from_start - TAB[0] } else { x + from_start };
    [tx, y - TAB[1] + 4.0, TAB[0], TAB[1]]
}

/// How many tabs fit on the window's top edge.
pub fn tabs_fit(window: Rect) -> usize {
    ((window[2] + 4.0) / (TAB[0] + 4.0)).floor().max(0.0) as usize
}

/// A node's frame for grid cell `at`, with the tree panned by `pan`; Arabic grows right to left.
pub fn node(inside: Rect, at: [i32; 2], pan: [f32; 2], rtl: bool) -> Rect {
    let [ix, iy, iw, _] = inside;
    let col = at[0] as f32 * CELL[0] + pan[0];
    let y = iy + at[1] as f32 * CELL[1] + pan[1];
    let x = if rtl { ix + iw - col - FRAME } else { ix + col };
    [x, y, FRAME, FRAME]
}

/// The pan that puts the tree's grid in the middle of `inside` when it fits, and its start
/// corner at the start otherwise.
pub fn centre_pan(inside: Rect, cells: [[i32; 2]; 2]) -> [f32; 2] {
    let [lo, hi] = cells;
    let span = |a: usize| (hi[a] - lo[a]) as f32 * CELL[a] + FRAME;
    let fit = |a: usize, room: f32| {
        let s = span(a);
        let start = -(lo[a] as f32) * CELL[a];
        if s <= room { start + (room - s) / 2.0 } else { start + 8.0 }
    };
    [fit(0, inside[2]), fit(1, inside[3])]
}

/// The two-segment connection between a needed node and the node needing it, as the
/// advancement lines run: across from the parent's middle, then to the child's middle.
pub fn link(parent: Rect, child: Rect) -> [[f32; 2]; 3] {
    let mid = |r: Rect| [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0];
    let (a, b) = (mid(parent), mid(child));
    let elbow = [(a[0] + b[0]) / 2.0, a[1]];
    [a, elbow, [elbow[0], b[1]]]
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: [f32; 2] = [427.0, 240.0];

    #[test]
    fn the_window_fills_the_screen_inside_its_margin() {
        let w = window(SCREEN);
        assert_eq!(w[0], MARGIN);
        assert!(w[2] / SCREEN[0] >= 0.88);
        assert_eq!(w[1] + w[3], SCREEN[1] - MARGIN);
        assert!(tab(w, 0, false)[1] >= MARGIN - 4.0);
    }

    #[test]
    fn arabic_mirrors_tabs_and_nodes() {
        let w = window(SCREEN);
        let i = inside(w);
        let (l, r) = (tab(w, 0, false), tab(w, 0, true));
        assert_eq!(l[0] - w[0], w[0] + w[2] - (r[0] + r[2]));
        let (nl, nr) = (node(i, [2, 1], [0.0, 0.0], false), node(i, [2, 1], [0.0, 0.0], true));
        assert_eq!(nl[0] - i[0], i[0] + i[2] - (nr[0] + nr[2]));
        assert_eq!(nl[1], nr[1]);
    }

    #[test]
    fn a_small_tree_is_centred() {
        let i = inside(window(SCREEN));
        let pan = centre_pan(i, [[0, 0], [2, 1]]);
        let first = node(i, [0, 0], pan, false);
        let last = node(i, [2, 1], pan, false);
        let left = first[0] - i[0];
        let right = i[0] + i[2] - (last[0] + last[2]);
        assert!((left - right).abs() < 0.01);
    }

    #[test]
    fn tabs_stay_on_the_window() {
        let w = window(SCREEN);
        let n = tabs_fit(w);
        assert!(n >= 5);
        for rtl in [false, true] {
            let t = tab(w, n - 1, rtl);
            assert!(t[0] >= w[0] && t[0] + t[2] <= w[0] + w[2]);
        }
    }
}
