mod text;

use flowmango::prelude::*;
use quartz::{Color, Font, FromSource, NamedKey, Shared, SourceSettings, TextSpec};
use image::RgbaImage;
use ramp::prism;

use text::SyntaxHighlighter;


#[derive(Clone)]
struct EditorSettings {
    font_size:                f32,
    line_height_ratio:        f32,
    char_width_ratio:         f32,
    number_padding_left:      f32,
    number_padding_right:     f32,
    gutter_columns:           f32,
    cursor_width:             f32,
    scroll_speed:             f32,
    scroll_speed_max:         f32,
    tab_size:                 usize,
    background:               String,
    background_gutter:        String,
    background_row_highlight: String,
    color_text:               String,
    color_line_number:        String,
    color_line_number_active: String,
    color_cursor:             String,
    color_gutter_separator:   String,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            font_size:                16.0,
            line_height_ratio:        1.5,
            char_width_ratio:         0.6,
            number_padding_left:      8.0,
            number_padding_right:     18.0,
            gutter_columns:           4.0,
            cursor_width:             2.0,
            scroll_speed:             1.0,
            scroll_speed_max:         50.0,
            tab_size:                 4,
            background:               "#12101a".into(),
            background_gutter:        "#1a1726".into(),
            background_row_highlight: "#221e30".into(),
            color_text:               "#e2dff0".into(),
            color_line_number:        "#6b6580".into(),
            color_line_number_active: "#c9b8ff".into(),
            color_cursor:             "#9d6bff".into(),
            color_gutter_separator:   "#2a2540".into(),
        }
    }
}

impl FromSource for EditorSettings {
    fn from_source(source_params: &SourceSettings) -> Self {
        let d = Self::default();
        Self {
            font_size:                source_params.f32("font_size")                .unwrap_or(d.font_size),
            line_height_ratio:        source_params.f32("line_height_ratio")        .unwrap_or(d.line_height_ratio),
            char_width_ratio:         source_params.f32("char_width_ratio")         .unwrap_or(d.char_width_ratio),
            number_padding_left:      source_params.f32("number_padding_left")      .unwrap_or(d.number_padding_left),
            number_padding_right:     source_params.f32("number_padding_right")     .unwrap_or(d.number_padding_right),
            gutter_columns:           source_params.f32("gutter_columns")           .unwrap_or(d.gutter_columns),
            cursor_width:             source_params.f32("cursor_width")             .unwrap_or(d.cursor_width),
            scroll_speed:             source_params.f32("scroll_speed")             .unwrap_or(d.scroll_speed),
            scroll_speed_max:         source_params.f32("scroll_speed_max")         .unwrap_or(d.scroll_speed_max),
            tab_size:                 source_params.usize("tab_size")               .unwrap_or(d.tab_size),
            background:               source_params.str("background")               .unwrap_or(d.background),
            background_gutter:        source_params.str("background_gutter")        .unwrap_or(d.background_gutter),
            background_row_highlight: source_params.str("background_row_highlight") .unwrap_or(d.background_row_highlight),
            color_text:               source_params.str("color_text")               .unwrap_or(d.color_text),
            color_line_number:        source_params.str("color_line_number")        .unwrap_or(d.color_line_number),
            color_line_number_active: source_params.str("color_line_number_active") .unwrap_or(d.color_line_number_active),
            color_cursor:             source_params.str("color_cursor")             .unwrap_or(d.color_cursor),
            color_gutter_separator:   source_params.str("color_gutter_separator")   .unwrap_or(d.color_gutter_separator),
        }
    }
}

impl EditorSettings {
    fn line_height(&self)  -> f32    { self.font_size * self.line_height_ratio }
    fn char_width(&self)   -> f32    { self.font_size * self.char_width_ratio }
    fn tab_string(&self)   -> String { " ".repeat(self.tab_size) }

    fn gutter_width_for(&self, line_count: usize) -> f32 {
        let digit_cols = if line_count == 0 { 1.0 }
                         else { (line_count as f32).log10().floor() + 1.0 };
        let cols = digit_cols.max(self.gutter_columns);
        self.char_width() * cols + self.number_padding_left + self.number_padding_right
    }
    fn text_start_x_for(&self, line_count: usize) -> f32 {
        self.gutter_width_for(line_count)
    }
}

fn needed_slots(view_height: f32, line_height: f32) -> usize {
    (view_height / line_height).ceil() as usize + 16
}

pub(crate) fn hex_to_color(hex: &str) -> Color {
    let trimmed = hex.trim_start_matches('#');
    let value   = u32::from_str_radix(trimmed, 16).unwrap_or(0);
    Color(
        ((value >> 16) & 0xFF) as u8,
        ((value >>  8) & 0xFF) as u8,
        ( value        & 0xFF) as u8,
        255,
    )
}

fn solid_color_image(width: f32, height: f32, color: Color) -> Image {
    let mut img = RgbaImage::new(1, 1);
    img.pixels_mut().for_each(|p| *p = image::Rgba([color.0, color.1, color.2, color.3]));
    Image {
        shape: ShapeType::Rectangle(0.0, (width, height), 0.0),
        image: img.into(),
        color: None,
    }
}

fn add_rectangle(
    canvas: &mut Canvas,
    name: &str, x: f32, y: f32, width: f32, height: f32, color: Color, tag: &str,
) {
    let mut obj = GameObject::build(name)
        .position(x, y).size(width, height).tag(tag).finish();
    obj.set_image(solid_color_image(width, height, color));
    canvas.add_game_object(name.to_string(), obj);
}

fn rebuild_chrome(
    canvas: &mut Canvas, s: &EditorSettings,
    vw: f32, vh: f32, line_count: usize,
) {
    let gw = s.gutter_width_for(line_count);
    let lh = s.line_height();
    macro_rules! upd {
        ($name:expr, $x:expr, $y:expr, $w:expr, $h:expr, $col:expr) => {
            if let Some(o) = canvas.get_game_object_mut($name) {
                o.position = ($x, $y);
                o.size     = ($w, $h);
                o.set_image(solid_color_image($w, $h, $col));
            }
        };
    }
    upd!("bg",      0.0,      0.0, vw,           vh,  hex_to_color(&s.background));
    upd!("gutter",  0.0,      0.0, gw,           vh,  hex_to_color(&s.background_gutter));
    upd!("gut_sep", gw - 1.0, 0.0, 1.0,          vh,  hex_to_color(&s.color_gutter_separator));
    upd!("row_hl",  gw,       0.0, vw - gw,      lh,  hex_to_color(&s.background_row_highlight));
    upd!("cursor",  gw,       0.0, s.cursor_width, lh, hex_to_color(&s.color_cursor));
}

fn ensure_slots(
    canvas: &mut Canvas,
    state:  &Shared<State>,
    s:      &EditorSettings,
    font:   &Font,
    vh:     f32,
) {
    let needed  = needed_slots(vh, s.line_height());
    let current = state.get().slot_count();
    if needed <= current { return; }

    let line_count = state.get().lines.len();

    for idx in current..needed {
        let lname = format!("line_{}", idx);
        let lspec = text::create_text_spec("", font, hex_to_color(&s.color_text), s.font_size);
        let mut lo = GameObject::build(&lname)
            .position(s.text_start_x_for(line_count), 0.0)
            .size(999_999.0, s.line_height())
            .tag("line")
            .finish();
        lo.set_text(lspec);
        canvas.add_game_object(lname, lo);

        let gname = format!("gnum_{}", idx);
        let gspec = text::create_text_spec("", font, hex_to_color(&s.color_line_number), s.font_size);
        let mut go = GameObject::build(&gname)
            .position(s.number_padding_left, 0.0)
            .size(s.gutter_width_for(line_count), s.line_height())
            .tag("gnum")
            .finish();
        go.set_text(gspec);
        canvas.add_game_object(gname, go);

        state.get_mut().slots.push(SlotCache::empty());
    }

    // Slots grew — force full text repaint.
    state.get_mut().dirty_all_text      = true;
    state.get_mut().needs_layout        = true;
    state.get_mut().dirty_gutters_from  = Some(0);
    state.get_mut().dirty_cursor_chrome = true;
}

// ── Per-slot cache ────────────────────────────────────────────────────────────

pub(crate) struct SlotCache {
    pub doc_row:   usize,
    pub text:      String,
    pub is_active: bool,
}

impl SlotCache {
    pub fn empty() -> Self {
        Self { doc_row: usize::MAX, text: String::new(), is_active: false }
    }
}

// ── State ─────────────────────────────────────────────────────────────────────

pub(crate) struct State {
    pub lines:         Vec<String>,
    pub cursor_row:    usize,
    pub cursor_column: usize,
    pub scroll_y:      f32,
        scroll_max:    f32,
    pub slots:         Vec<SlotCache>,
    pub first_row:     usize,      // usize::MAX = unknown

        snap_cursor:   bool,
    pub last_view_width:  f32,
    pub last_view_height: f32,

    // Per-document-row highlight cache. Indexed by doc row.
    // None = needs rehighlight, Some = cached TextSpec ready to blit.
    pub highlight_cache: Vec<Option<TextSpec>>,

    // Dirty flags — set by mutation methods, consumed once per frame by flush().
    pub needs_layout:              bool,
    pub needs_chrome_reposition:   bool,
    pub dirty_cursor_chrome:       bool,
    pub dirty_doc_rows:            Vec<usize>,
    pub dirty_all_text:            bool,
    pub dirty_gutters_from:        Option<usize>,
    pub dirty_gutter_deactivate:   Option<usize>,
    pub dirty_gutter_activate:     Option<usize>,
}

impl State {
    fn new(slot_count: usize) -> Self {
        Self {
            lines:          vec![String::new()],
            cursor_row:     0,
            cursor_column:  0,
            scroll_y:       0.0,
            scroll_max:     0.0,
            slots:          (0..slot_count).map(|_| SlotCache::empty()).collect(),
            first_row:      usize::MAX,
            snap_cursor:    false,
            last_view_width:  0.0,
            last_view_height: 0.0,
            highlight_cache: vec![None],  // one entry per line
            // Everything dirty on first frame.
            needs_layout:              true,
            needs_chrome_reposition:   true,
            dirty_cursor_chrome:       true,
            dirty_doc_rows:            Vec::new(),
            dirty_all_text:            true,
            dirty_gutters_from:        Some(0),
            dirty_gutter_deactivate:   None,
            dirty_gutter_activate:     Some(0),
        }
    }

    pub fn slot_count(&self) -> usize { self.slots.len() }

    /// Invalidate the highlight cache for every row from `from_row` onward.
    /// Called after structural edits (insert/remove lines) where syntax context
    /// may have changed for all subsequent lines.
    fn invalidate_highlight_from(&mut self, from_row: usize) {
        for i in from_row..self.highlight_cache.len() {
            self.highlight_cache[i] = None;
        }
    }

    /// Invalidate the highlight cache for a single row.
    fn invalidate_highlight_row(&mut self, row: usize) {
        if row < self.highlight_cache.len() {
            self.highlight_cache[row] = None;
        }
    }

    /// Clear the entire highlight cache (e.g. on theme/settings change).
    fn invalidate_highlight_all(&mut self) {
        for entry in &mut self.highlight_cache {
            *entry = None;
        }
    }

    fn invalidate_slots_from(&mut self, from_row: usize) {
        for slot in &mut self.slots {
            if slot.doc_row != usize::MAX && slot.doc_row >= from_row {
                *slot = SlotCache::empty();
            }
        }
    }

    fn update_scroll_max(&mut self, s: &EditorSettings, vh: f32) {
        let content_h  = self.lines.len() as f32 * s.line_height();
        self.scroll_max = (content_h - vh).max(0.0);
        self.scroll_y   = self.scroll_y.clamp(0.0, self.scroll_max);
    }

    fn scroll_by(&mut self, delta: f32, speed_max: f32) {
        let prev      = self.scroll_y;
        self.scroll_y = (self.scroll_y + delta.clamp(-speed_max, speed_max))
            .clamp(0.0, self.scroll_max);
        if (self.scroll_y - prev).abs() > 0.01 {
            self.needs_layout        = true;
            self.dirty_cursor_chrome = true;
        }
    }

    fn ensure_cursor_visible(&mut self, s: &EditorSettings, vh: f32) {
        let top    = self.cursor_row as f32 * s.line_height();
        let bottom = top + s.line_height();
        if top < self.scroll_y             { self.scroll_y = top; }
        else if bottom > self.scroll_y + vh { self.scroll_y = bottom - vh; }
        self.scroll_y = self.scroll_y.clamp(0.0, self.scroll_max);
    }

    fn click(&mut self, click_x: f32, click_y: f32, s: &EditorSettings) {
        let lc = self.lines.len();
        if click_x < s.gutter_width_for(lc) { return; }
        let row = ((click_y + self.scroll_y) / s.line_height()).floor() as usize;
        let row = row.min(lc.saturating_sub(1));
        let cf  = ((click_x - s.text_start_x_for(lc)) / s.char_width()).round();
        let col = (if cf < 0.0 { 0 } else { cf as usize })
            .min(self.lines[row].chars().count());
        let prev = self.cursor_row;
        self.cursor_row    = row;
        self.cursor_column = col;
        self.on_cursor_moved(prev);
    }

    fn insert_str(&mut self, text: &str) {
        let row  = self.cursor_row;
        let bi   = self.char_to_byte(row, self.cursor_column);
        self.lines[row].insert_str(bi, text);
        self.cursor_column   += text.chars().count();
        self.invalidate_highlight_row(row);
        self.dirty_doc_rows.push(row);
        self.snap_cursor         = true;
        self.dirty_cursor_chrome = true;
    }

    fn backspace(&mut self) {
        if self.cursor_column > 0 {
            let row = self.cursor_row;
            let bs  = self.char_to_byte(row, self.cursor_column - 1);
            let be  = self.char_to_byte(row, self.cursor_column);
            self.lines[row].drain(bs..be);
            self.cursor_column  -= 1;
            self.invalidate_highlight_row(row);
            self.dirty_doc_rows.push(row);
            self.snap_cursor         = true;
            self.dirty_cursor_chrome = true;
        } else if self.cursor_row > 0 {
            let row       = self.cursor_row;
            let remainder = self.lines.remove(row);
            let prev_row  = row - 1;
            let prev_len  = self.lines[prev_row].chars().count();
            self.lines[prev_row].push_str(&remainder);
            // Remove the deleted row's cache entry and invalidate from prev_row onward.
            self.highlight_cache.remove(row);
            self.invalidate_highlight_from(prev_row);
            let old_cursor    = self.cursor_row;
            self.cursor_row   = prev_row;
            self.cursor_column = prev_len;
            self.on_structural_edit(prev_row, old_cursor);
        }
    }

    fn enter(&mut self) {
        let row  = self.cursor_row;
        let bi   = self.char_to_byte(row, self.cursor_column);
        let rest = self.lines[row].split_off(bi);
        let old  = self.cursor_row;
        self.cursor_row    += 1;
        self.cursor_column  = 0;
        self.lines.insert(self.cursor_row, rest);
        // Insert a new cache entry for the new line and invalidate from the split onward.
        self.highlight_cache.insert(self.cursor_row, None);
        self.invalidate_highlight_from(row);
        self.on_structural_edit(row, old);
    }

    fn on_structural_edit(&mut self, first_changed: usize, old_cursor_row: usize) {
        self.invalidate_slots_from(first_changed);
        self.dirty_gutters_from = Some(
            self.dirty_gutters_from.map_or(first_changed, |r| r.min(first_changed))
        );
        self.needs_layout              = true;
        self.needs_chrome_reposition   = true;
        self.snap_cursor               = true;
        self.dirty_cursor_chrome       = true;
        if old_cursor_row != self.cursor_row {
            self.dirty_gutter_deactivate = Some(old_cursor_row);
            self.dirty_gutter_activate   = Some(self.cursor_row);
        }
    }

    fn move_left(&mut self) {
        let prev = self.cursor_row;
        if self.cursor_column > 0 {
            self.cursor_column -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row   -= 1;
            self.cursor_column = self.lines[self.cursor_row].chars().count();
        }
        self.on_cursor_moved(prev);
    }

    fn move_right(&mut self) {
        let prev = self.cursor_row;
        let len  = self.lines[self.cursor_row].chars().count();
        if self.cursor_column < len {
            self.cursor_column += 1;
        } else if self.cursor_row < self.lines.len() - 1 {
            self.cursor_row    += 1;
            self.cursor_column  = 0;
        }
        self.on_cursor_moved(prev);
    }

    fn move_up(&mut self) {
        let prev = self.cursor_row;
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.clamp_column();
        }
        self.on_cursor_moved(prev);
    }

    fn move_down(&mut self) {
        let prev = self.cursor_row;
        if self.cursor_row < self.lines.len() - 1 {
            self.cursor_row += 1;
            self.clamp_column();
        }
        self.on_cursor_moved(prev);
    }

    fn on_cursor_moved(&mut self, prev_row: usize) {
        self.snap_cursor         = true;
        self.dirty_cursor_chrome = true;
        if prev_row != self.cursor_row {
            self.dirty_gutter_deactivate = Some(prev_row);
            self.dirty_gutter_activate   = Some(self.cursor_row);
        }
    }

    fn char_to_byte(&self, row: usize, char_index: usize) -> usize {
        self.lines[row]
            .char_indices()
            .nth(char_index)
            .map(|(b, _)| b)
            .unwrap_or(self.lines[row].len())
    }

    fn clamp_column(&mut self) {
        self.cursor_column = self.cursor_column
            .min(self.lines[self.cursor_row].chars().count());
    }
}

pub struct App;

const SOURCE_FILE: &str = "src/lib.rs";

impl App {
    pub fn new(context: &mut Context, assets: Assets) -> Scene {
        let settings = Shared::new(EditorSettings::default());

        let font_bytes = assets.get_font("JetBrainsMono-Bold.ttf").expect("font");
        let font       = Font::from_bytes(&font_bytes).expect("invalid font");

        let highlighter = SyntaxHighlighter::new();

        let mut scene        = Scene::new(context, CanvasMode::Fullscreen, 1);
        let layer_id         = LayerId(0);
        let (vw, vh)         = scene.get_virtual_size();
        let canvas           = scene.get_layer_mut(layer_id).unwrap().canvas_mut();
        let initial_slots    = needed_slots(vh, settings.get().line_height());
        let state            = Shared::new(State::new(initial_slots));

        {
            let s  = settings.get();
            let gw = s.gutter_width_for(1);
            let lh = s.line_height();

            add_rectangle(canvas, "bg",      0.0,      0.0, vw,           vh,  hex_to_color(&s.background),               "chrome");
            add_rectangle(canvas, "gutter",  0.0,      0.0, gw,           vh,  hex_to_color(&s.background_gutter),        "chrome");
            add_rectangle(canvas, "gut_sep", gw - 1.0, 0.0, 1.0,          vh,  hex_to_color(&s.color_gutter_separator),   "chrome");
            add_rectangle(canvas, "row_hl",  gw,       0.0, vw - gw,      lh,  hex_to_color(&s.background_row_highlight), "chrome");
            add_rectangle(canvas, "cursor",  gw,       0.0, s.cursor_width, lh, hex_to_color(&s.color_cursor),             "chrome");

            for idx in 0..initial_slots {
                let ty = idx as f32 * lh;

                let lname = format!("line_{}", idx);
                let lspec = text::create_text_spec("", &font, hex_to_color(&s.color_text), s.font_size);
                let mut lo = GameObject::build(&lname)
                    .position(s.text_start_x_for(1), ty)
                    .size(999_999.0, lh)
                    .tag("line")
                    .finish();
                lo.set_text(lspec);
                canvas.add_game_object(lname, lo);

                let gname = format!("gnum_{}", idx);
                let gspec = text::create_text_spec("", &font, hex_to_color(&s.color_line_number), s.font_size);
                let mut go = GameObject::build(&gname)
                    .position(s.number_padding_left, ty)
                    .size(gw, lh)
                    .tag("gnum")
                    .finish();
                go.set_text(gspec);
                canvas.add_game_object(gname, go);
            }
        }

        canvas.watch_source(SOURCE_FILE, settings.clone());

        let state_k = state.clone();
        let settings_k = settings.clone();
        canvas.on_key_press(move |_cv, key| {
            let tab = settings_k.get().tab_string();
            let mut st = state_k.get_mut();
            match key {
                Key::Named(NamedKey::Enter)      => st.enter(),
                Key::Named(NamedKey::Space)      => st.insert_str(" "),
                Key::Named(NamedKey::Delete)     => st.backspace(),
                Key::Named(NamedKey::ArrowLeft)  => st.move_left(),
                Key::Named(NamedKey::ArrowRight) => st.move_right(),
                Key::Named(NamedKey::ArrowUp)    => st.move_up(),
                Key::Named(NamedKey::ArrowDown)  => st.move_down(),
                Key::Named(NamedKey::Tab)        => st.insert_str(&tab),
                Key::Named(_)                    => {}
                Key::Character(ch) => {
                    if ch.as_str() == "\u{8}" || ch.as_str() == "\x7f" {
                        st.backspace();
                    } else if ch.chars().all(|c| !c.is_control()) {
                        st.insert_str(ch.as_str());
                    }
                }
            }
        });

        let state_c = state.clone();
        let settings_c = settings.clone();
        canvas.on_mouse_press(move |_cv, _btn, (cx, cy)| {
            let s = settings_c.get();
            state_c.get_mut().click(cx, cy, &s);
        });

        let state_s = state.clone();
        let settings_s = settings.clone();
        canvas.on_mouse_scroll(move |_cv, (_dx, dy)| {
            let s = settings_s.get();
            state_s.get_mut().scroll_by(dy * s.scroll_speed, s.scroll_speed_max);
        });

        let font_t        = font.clone();
        let settings_t    = settings.clone();
        let highlighter_t = highlighter.clone();
        canvas.on_update(move |canvas| {
            let (vw, vh) = canvas.get_virtual_size();

            // ── resize ────────────────────────────────────────────────────────
            let size_changed = {
                let st = state.get();
                (vw - st.last_view_width).abs() > 0.5 || (vh - st.last_view_height).abs() > 0.5
            };
            if size_changed {
                {
                    let mut st        = state.get_mut();
                    st.last_view_width  = vw;
                    st.last_view_height = vh;
                    st.needs_layout              = true;
                    st.needs_chrome_reposition   = true;
                }
                let s = settings_t.get();
                ensure_slots(canvas, &state, &s, &font_t, vh);
            }

            // ── settings hot-reload ───────────────────────────────────────────
            if settings_t.changed() {
                let s  = settings_t.get();
                let lc = state.get().lines.len();
                rebuild_chrome(canvas, &s, vw, vh, lc);
                // Force full text and gutter repaint; positions also need refresh.
                let mut st = state.get_mut();
                st.dirty_all_text            = true;
                st.needs_layout              = true;
                st.needs_chrome_reposition   = true;
                st.dirty_gutters_from        = Some(0);
                st.dirty_cursor_chrome       = true;
                // Invalidate all highlight caches since theme/font may have changed.
                st.invalidate_highlight_all();
                for slot in &mut st.slots {
                    *slot = SlotCache::empty();
                }
            }

            // ── snap cursor into view ─────────────────────────────────────────
            let snap = state.get().snap_cursor;
            if snap {
                let s = settings_t.get();
                let mut st = state.get_mut();
                st.update_scroll_max(&s, vh);
                st.ensure_cursor_visible(&s, vh);
                st.snap_cursor         = false;
                st.needs_layout        = true;
                st.dirty_cursor_chrome = true;
            } else {
                // Still update scroll max in case lines were added/removed.
                let s  = settings_t.get();
                let mut st = state.get_mut();
                st.update_scroll_max(&s, vh);
            }

            // ── flush dirty work to canvas ────────────────────────────────────
            text::flush(
                canvas, &state, &settings_t.get(), &font_t, &highlighter_t, vw, vh,
            );
        });

        scene
    }
}

ramp::run! { |context: &mut Context, assets: Assets| {
    App::new(context, assets)
}}