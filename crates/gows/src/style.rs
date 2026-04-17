use iced::Color;

pub(crate) const PAGE_SIZE: usize = 200;
pub(crate) const ROW_HEIGHT: f32 = 24.0;
pub(crate) const LANE_WIDTH: f32 = 16.0;
pub(crate) const DOT_RADIUS: f32 = 4.0;
pub(crate) const MIN_COL_WIDTH: f32 = 40.0;
pub(crate) const DEFAULT_HASH_WIDTH: f32 = 80.0;
pub(crate) const DEFAULT_DATE_WIDTH: f32 = 180.0;
pub(crate) const DEFAULT_AUTHOR_WIDTH: f32 = 120.0;
pub(crate) const HANDLE_WIDTH: f32 = 8.0;

// ---------------------------------------------------------------------------
// Graph colors — Catppuccin inspired palette
// ---------------------------------------------------------------------------

pub(crate) const GRAPH_COLORS: &[Color] = &[
    Color {
        r: 0.537,
        g: 0.706,
        b: 0.980,
        a: 1.0,
    }, // blue
    Color {
        r: 0.651,
        g: 0.890,
        b: 0.631,
        a: 1.0,
    }, // green
    Color {
        r: 0.976,
        g: 0.886,
        b: 0.686,
        a: 1.0,
    }, // yellow
    Color {
        r: 0.953,
        g: 0.545,
        b: 0.659,
        a: 1.0,
    }, // red/pink
    Color {
        r: 0.796,
        g: 0.651,
        b: 0.969,
        a: 1.0,
    }, // purple
    Color {
        r: 0.580,
        g: 0.886,
        b: 0.835,
        a: 1.0,
    }, // teal
    Color {
        r: 0.980,
        g: 0.702,
        b: 0.529,
        a: 1.0,
    }, // peach
    Color {
        r: 0.455,
        g: 0.780,
        b: 0.925,
        a: 1.0,
    }, // sapphire
    Color {
        r: 0.949,
        g: 0.804,
        b: 0.804,
        a: 1.0,
    }, // flamingo
    Color {
        r: 0.706,
        g: 0.745,
        b: 0.996,
        a: 1.0,
    }, // lavender
];

pub(crate) fn lane_color(idx: usize) -> Color {
    GRAPH_COLORS[idx % GRAPH_COLORS.len()]
}

// ---------------------------------------------------------------------------
// Column resize state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub(crate) enum ResizeHandle {
    Graph,
    Hash,
    Date,
    Author,
}

pub(crate) struct DragState {
    pub(crate) handle: ResizeHandle,
    pub(crate) start_x: Option<f32>,
    pub(crate) start_width: f32,
}

pub(crate) struct ContextMenu {
    pub(crate) commit_index: usize,
}

pub(crate) struct InspectState {
    pub(crate) detail: String,
}
