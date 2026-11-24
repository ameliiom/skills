//! The screen's geometry and sprites, in the advancements screen's own measures. Pure: the
//! drawing (`screen.rs`, build stage) only reads these.

/// `[x, y, width, height]` in GUI pixels.
pub type Rect = [f32; 4];

/// The advancements screen's measures (`AdvancementsScreen`, `AdvancementTabType`, `AdvancementWidget`).
pub const BORDER: [f32; 4] = [9.0, 18.0, 9.0, 9.0]; // start, top, end, bottom of the window frame
pub const TAB: [f32; 2] = [28.0, 32.0];
pub const FRAME: f32 = 26.0;
/// A grid cell: a frame and the gap its links show in (vanilla packs its denser trees at 28 × 27).
pub const CELL: [f32; 2] = [44.0, 36.0];
/// The gap kept around the window, so the world shows at the screen's edge.
pub const MARGIN: f32 = 16.0;

/// The game's sprites the screen draws with (`textures/gui/sprites/…`), names read from the
/// game's assets.
pub mod sprite {
    pub const FRAME_UNLOCKED: &str = "advancements/task_frame_obtained";
    pub const FRAME: &str = "advancements/task_frame_unobtained";
    pub const XP_BACK: &str = "hud/experience_bar_background";
    pub const XP_FILL: &str = "hud/experience_bar_progress";
    /// The window frame is a plain texture (252×140), drawn nine-slice by its 9/18/9/9 border.
    pub const WINDOW: &str = "gui/advancements/window";
    /// The advancement toast's frame.
    pub const TOAST: &str = "toast/advancement";

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

/// Whether `point` is inside `rect`.
pub fn contains([x, y, w, h]: Rect, [px, py]: [f32; 2]) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}

/// The lowest and highest grid cells of a tree (`[[0, 0], [0, 0]]` for none).
pub fn bounds(cells: impl Iterator<Item = [i32; 2]>) -> [[i32; 2]; 2] {
    cells
        .fold(None, |acc: Option<[[i32; 2]; 2]>, c| {
            Some(match acc {
                None => [c, c],
                Some([lo, hi]) => [[lo[0].min(c[0]), lo[1].min(c[1])], [hi[0].max(c[0]), hi[1].max(c[1])]],
            })
        })
        .unwrap_or([[0, 0], [0, 0]])
}

/// How vanilla's attribute tooltip says a modifier: its translation key, the amount as shown
/// (a percentage for the multiplying operations) and whether it reads as a gain (blue) or a loss.
pub fn modifier_text(amount: f64, operation: crate::tree::Operation) -> (String, String, bool) {
    use crate::tree::Operation;
    let (n, shown) = match operation {
        Operation::AddValue => (0, amount),
        Operation::AddMultipliedBase => (1, amount * 100.0),
        Operation::AddMultipliedTotal => (2, amount * 100.0),
    };
    let sign = if amount >= 0.0 { "plus" } else { "take" };
    let value = format!("{:.2}", shown.abs());
    let value = value.trim_end_matches('0').trim_end_matches('.').to_owned();
    (format!("attribute.modifier.{sign}.{n}"), value, amount >= 0.0)
}

/// The window's title row: where the tree's name may go, and the text's top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Header {
    pub title_start: f32,
    pub title_end: f32,
    pub y: f32,
}

/// Vanilla draws the window title at (8, 6) inside the frame.
pub fn header([x, y, w, _]: Rect, _rtl: bool) -> Header {
    Header { title_start: x + 8.0, title_end: x + w - 8.0, y: y + 6.0 }
}

/// Where the level number, the experience bar and the points go at the title row's end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeaderEnd {
    pub level: [f32; 2],
    pub bar: Rect,
    pub points: [f32; 2],
}

/// The experience bar's length in the header: the HUD's own 182, never scaled.
pub const BAR: f32 = 182.0;

/// Laid out from the end of the row: points, the bar, the level number (Arabic mirrors it).
pub fn header_end(h: Header, level_w: f32, points_w: f32, rtl: bool) -> HeaderEnd {
    // In reading order from the row's end inwards, at LTR coordinates measured from the end.
    let points_x = points_w;
    let bar_x = points_x + 8.0 + BAR;
    let level_x = bar_x + 4.0 + level_w;
    let at = |from_end: f32, width: f32| if rtl { h.title_start + from_end - width } else { h.title_end - from_end };
    HeaderEnd {
        level: [at(level_x, level_w), h.y],
        bar: [at(bar_x, BAR), h.y + 1.5, BAR, 5.0],
        points: [at(points_x, points_w), h.y],
    }
}

/// How far a toast has slid in (0 hidden, 1 fully shown) at `age` ticks of `life`.
pub fn toast_slide(age: f32, life: f32, slide: f32) -> f32 {
    (age / slide).min((life - age) / slide).clamp(0.0, 1.0)
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
    fn modifiers_read_as_vanilla_says_them() {
        use crate::tree::Operation;
        assert_eq!(modifier_text(0.5, Operation::AddValue), ("attribute.modifier.plus.0".into(), "0.5".into(), true));
        assert_eq!(modifier_text(0.1, Operation::AddMultipliedBase), ("attribute.modifier.plus.1".into(), "10".into(), true));
        assert_eq!(modifier_text(-0.25, Operation::AddValue), ("attribute.modifier.take.0".into(), "0.25".into(), false));
        assert_eq!(modifier_text(2.0, Operation::AddValue).1, "2");
    }

    #[test]
    fn the_header_end_mirrors_and_never_meets_the_title() {
        let w = window(SCREEN);
        for rtl in [false, true] {
            let h = header(w, rtl);
            let e = header_end(h, 12.0, 40.0, rtl);
            let lo = e.level[0].min(e.points[0]).min(e.bar[0]);
            let hi = (e.level[0] + 12.0).max(e.points[0] + 40.0).max(e.bar[0] + BAR);
            assert!(lo >= h.title_start && hi <= h.title_end);
            // No overlap between the three.
            let spans = [(e.level[0], 12.0), (e.bar[0], BAR), (e.points[0], 40.0)];
            for (i, a) in spans.iter().enumerate() {
                for b in &spans[i + 1..] {
                    assert!(a.0 + a.1 <= b.0 || b.0 + b.1 <= a.0, "{a:?} {b:?}");
                }
            }
        }
    }

    #[test]
    fn toasts_slide_in_and_out() {
        assert_eq!(toast_slide(0.0, 100.0, 6.0), 0.0);
        assert_eq!(toast_slide(50.0, 100.0, 6.0), 1.0);
        assert_eq!(toast_slide(100.0, 100.0, 6.0), 0.0);
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
