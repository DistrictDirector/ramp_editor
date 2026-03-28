mod text;

use flowmango::prelude::*;
use quartz::{Color, Font, FromSource, NamedKey, Shared, SourceSettings};
use image::RgbaImage;
use ramp::prism;


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
    tab_size:                 usize,
    visible_rows:             usize,
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
            line_height_ratio:        1.4,
            char_width_ratio:         0.6,
            number_padding_left:      4.0,
            number_padding_right:     18.0,
            gutter_columns:           4.0,
            cursor_width:             2.0,
            scroll_speed:             1.0,
            tab_size:                 4,
            visible_rows:             60,
            background:               "#1e1e2e".into(),
            background_gutter:        "#181825".into(),
            background_row_highlight: "#2a2a3d".into(),
            color_text:               "#cdd6f4".into(),
            color_line_number:        "#45475a".into(),
            color_line_number_active: "#cba6f7".into(),
            color_cursor:             "#cba6f7".into(),
            color_gutter_separator:   "#ffffff".into(),
        }
    }
}

impl FromSource for EditorSettings {
    fn from_source(source_params: &SourceSettings) -> Self {
        let defaults = Self::default();
        Self {
            font_size:                source_params.f32("font_size")    .unwrap_or(defaults.font_size),
            line_height_ratio:        source_params.f32("line_height")  .unwrap_or(defaults.line_height_ratio),
            char_width_ratio:         source_params.f32("char_width")   .unwrap_or(defaults.char_width_ratio),
            number_padding_left:      source_params.f32("num_pad_l")    .unwrap_or(defaults.number_padding_left),
            number_padding_right:     source_params.f32("num_pad_r")    .unwrap_or(defaults.number_padding_right),
            gutter_columns:           source_params.f32("gutter_cols")  .unwrap_or(defaults.gutter_columns),
            cursor_width:             source_params.f32("cursor_width") .unwrap_or(defaults.cursor_width),
            scroll_speed:             source_params.f32("scroll_speed") .unwrap_or(defaults.scroll_speed),
            tab_size:                 source_params.usize("tab_size")   .unwrap_or(defaults.tab_size),
            visible_rows:             defaults.visible_rows,
            background:               source_params.str("bg")           .unwrap_or(defaults.background),
            background_gutter:        source_params.str("bg_gutter")    .unwrap_or(defaults.background_gutter),
            background_row_highlight: source_params.str("bg_row_hl")    .unwrap_or(defaults.background_row_highlight),
            color_text:               source_params.str("col_text")     .unwrap_or(defaults.color_text),
            color_line_number:        source_params.str("col_lnum")     .unwrap_or(defaults.color_line_number),
            color_line_number_active: source_params.str("col_lnum_act") .unwrap_or(defaults.color_line_number_active),
            color_cursor:             source_params.str("col_cursor")   .unwrap_or(defaults.color_cursor),
            color_gutter_separator:   source_params.str("col_gut_sep")  .unwrap_or(defaults.color_gutter_separator),
        }
    }
}

impl EditorSettings {
    fn line_height(&self)  -> f32    { self.font_size * self.line_height_ratio }
    fn char_width(&self)   -> f32    { self.font_size * self.char_width_ratio }
    fn gutter_width(&self) -> f32    { self.char_width() * self.gutter_columns + self.number_padding_left + self.number_padding_right }
    fn text_start_x(&self) -> f32    { self.gutter_width() }
    fn tab_string(&self)   -> String { " ".repeat(self.tab_size) }
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
    let mut image = RgbaImage::new(1, 1);
    image.pixels_mut().for_each(|pixel| {
        *pixel = image::Rgba([color.0, color.1, color.2, color.3]);
    });
    Image {
        shape: ShapeType::Rectangle(0.0, (width, height), 0.0),
        image: image.into(),
        color: None,
    }
}

fn add_rectangle(
    canvas: &mut Canvas,
    name:   &str,
    x:      f32,
    y:      f32,
    width:  f32,
    height: f32,
    color:  Color,
    tag:    &str,
) {
    let mut object = GameObject::build(name)
        .position(x, y)
        .size(width, height)
        .tag(tag)
        .finish();
    object.set_image(solid_color_image(width, height, color));
    canvas.add_game_object(name.to_string(), object);
}

fn set_bounds(canvas: &mut Canvas, name: &str, x: f32, y: f32, width: f32, height: f32) {
    if let Some(object) = canvas.get_game_object_mut(name) {
        object.size     = (width, height);
        object.position = (x, y);
    }
}

fn rebuild_chrome(canvas: &mut Canvas, settings: &EditorSettings, view_width: f32, view_height: f32) {
    let mut update = |name: &str, x: f32, y: f32, width: f32, height: f32, color: Color| {
        if let Some(object) = canvas.get_game_object_mut(name) {
            object.position = (x, y);
            object.size     = (width, height);
            object.set_image(solid_color_image(width, height, color));
        }
    };

    update("bg",      0.0,                           0.0, view_width,                            view_height,            hex_to_color(&settings.background));
    update("gutter",  0.0,                           0.0, settings.gutter_width(),               view_height,            hex_to_color(&settings.background_gutter));
    update("gut_sep", settings.gutter_width() - 1.0, 0.0, 1.0,                                   view_height,            hex_to_color(&settings.color_gutter_separator));
    update("row_hl",  settings.gutter_width(),        0.0, view_width - settings.gutter_width(),  settings.line_height(), hex_to_color(&settings.background_row_highlight));
    update("cursor",  settings.text_start_x(),        0.0, settings.cursor_width,                settings.line_height(), hex_to_color(&settings.color_cursor));
}

struct State {
    lines:                           Vec<String>,
    cursor_row:                      usize,
    cursor_column:                   usize,
    scroll_y:                        f32,
    scroll_max:                      f32,
    first_row:                       usize,
    revision:                        u64,
    last_revision:                   u64,
    last_scroll:                     f32,
    last_cursor_row:                 usize,
    last_cursor_column:              usize,
    line_names:                      Vec<String>,
    gutter_number_names:             Vec<String>,
    snap_cursor:                     bool,
    last_edited_row:                 Option<usize>,
    cached_line_text:                Vec<String>,
    cached_gutter_number_text:       Vec<String>,
    cached_gutter_number_is_current: Vec<bool>,
    last_view_width:                 f32,
    last_view_height:                f32,
    render_slot:                     usize,
    pending_render:                  bool,
}

impl State {
    fn new(settings: &EditorSettings) -> Self {
        let line_names          = (0..settings.visible_rows).map(|i| format!("line_{}", i)).collect();
        let gutter_number_names = (0..settings.visible_rows).map(|i| format!("gnum_{}", i)).collect();
        Self {
            lines:                           vec![String::new()],
            cursor_row:                      0,
            cursor_column:                   0,
            scroll_y:                        0.0,
            scroll_max:                      0.0,
            first_row:                       usize::MAX,
            revision:                        1,
            last_revision:                   0,
            last_scroll:                     f32::MAX,
            last_cursor_row:                 usize::MAX,
            last_cursor_column:              usize::MAX,
            line_names,
            gutter_number_names,
            snap_cursor:                     false,
            last_edited_row:                 None,
            cached_line_text:                vec![String::new(); settings.visible_rows],
            cached_gutter_number_text:       vec![String::new(); settings.visible_rows],
            cached_gutter_number_is_current: vec![false;         settings.visible_rows],
            last_view_width:                 0.0,
            last_view_height:                0.0,
            render_slot:                     0,
            pending_render:                  false,
        }
    }

    fn bump(&mut self)                  { self.revision = self.revision.wrapping_add(1); }
    fn bump_snap(&mut self)             { self.bump(); self.snap_cursor = true; }
    fn bump_edit(&mut self, row: usize) { self.last_edited_row = Some(row); self.start_render(); self.bump_snap(); }
    fn bump_structural(&mut self)       { self.last_edited_row = None;      self.start_render(); self.bump_snap(); }

    fn start_render(&mut self) {
        self.render_slot   = 0;
        self.pending_render = true;
    }

    fn invalidate_all(&mut self) {
        self.cached_line_text.iter_mut().for_each(|t| t.clear());
        self.cached_gutter_number_text.iter_mut().for_each(|t| t.clear());
        self.cached_gutter_number_is_current.iter_mut().for_each(|f| *f = false);
        self.first_row   = usize::MAX;
        self.last_scroll = f32::MAX;
        self.start_render();
        self.bump_structural();
    }

    fn update_scroll_max(&mut self, settings: &EditorSettings, view_height: f32) {
        let content_height = self.lines.len() as f32 * settings.line_height();
        self.scroll_max    = (content_height - view_height).max(0.0);
        self.scroll_y      = self.scroll_y.clamp(0.0, self.scroll_max);
    }

    fn scroll_by(&mut self, delta: f32) {
        self.scroll_y = (self.scroll_y + delta).clamp(0.0, self.scroll_max);
    }

    fn ensure_cursor_visible(&mut self, settings: &EditorSettings, view_height: f32) {
        let cursor_top    = self.cursor_row as f32 * settings.line_height();
        let cursor_bottom = cursor_top + settings.line_height();
        if cursor_top < self.scroll_y                       { self.scroll_y = cursor_top; }
        else if cursor_bottom > self.scroll_y + view_height { self.scroll_y = cursor_bottom - view_height; }
        self.scroll_y = self.scroll_y.clamp(0.0, self.scroll_max);
    }

    fn click(&mut self, click_x: f32, click_y: f32, settings: &EditorSettings) {
        if click_x < settings.gutter_width() { return; }
        let row      = ((click_y + self.scroll_y) / settings.line_height()).floor() as usize;
        let row      = row.min(self.lines.len().saturating_sub(1));
        let column_f = ((click_x - settings.text_start_x()) / settings.char_width()).round();
        let column   = (if column_f < 0.0 { 0 } else { column_f as usize })
                           .min(self.lines[row].chars().count());
        self.cursor_row    = row;
        self.cursor_column = column;
        self.bump_snap();
    }

    fn insert_str(&mut self, text: &str) {
        let row        = self.cursor_row;
        let byte_index = self.char_to_byte(row, self.cursor_column);
        self.lines[row].insert_str(byte_index, text);
        self.cursor_column += text.chars().count();
        self.bump_edit(row);
    }

    fn backspace(&mut self) {
        if self.cursor_column > 0 {
            let row        = self.cursor_row;
            let byte_start = self.char_to_byte(row, self.cursor_column - 1);
            let byte_end   = self.char_to_byte(row, self.cursor_column);
            self.lines[row].drain(byte_start..byte_end);
            self.cursor_column -= 1;
            self.bump_edit(row);
        } else if self.cursor_row > 0 {
            let row             = self.cursor_row;
            let remainder       = self.lines.remove(row);
            let previous_row    = row - 1;
            let previous_length = self.lines[previous_row].chars().count();
            self.lines[previous_row].push_str(&remainder);
            self.cursor_row    = previous_row;
            self.cursor_column = previous_length;
            self.bump_structural();
        }
    }

    fn enter(&mut self) {
        let row        = self.cursor_row;
        let byte_index = self.char_to_byte(row, self.cursor_column);
        let remainder  = self.lines[row].split_off(byte_index);
        self.cursor_row    += 1;
        self.cursor_column  = 0;
        self.lines.insert(self.cursor_row, remainder);
        self.bump_structural();
    }

    fn move_left(&mut self) {
        if self.cursor_column > 0 {
            self.cursor_column -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row    -= 1;
            self.cursor_column  = self.lines[self.cursor_row].chars().count();
        }
        self.bump_snap();
    }

    fn move_right(&mut self) {
        let line_length = self.lines[self.cursor_row].chars().count();
        if self.cursor_column < line_length {
            self.cursor_column += 1;
        } else if self.cursor_row < self.lines.len() - 1 {
            self.cursor_row    += 1;
            self.cursor_column  = 0;
        }
        self.bump_snap();
    }

    fn move_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.clamp_column();
        }
        self.bump_snap();
    }

    fn move_down(&mut self) {
        if self.cursor_row < self.lines.len() - 1 {
            self.cursor_row += 1;
            self.clamp_column();
        }
        self.bump_snap();
    }

    fn char_to_byte(&self, row: usize, char_index: usize) -> usize {
        self.lines[row]
            .char_indices()
            .nth(char_index)
            .map(|(byte_index, _)| byte_index)
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
        let state    = Shared::new(State::new(&settings.get()));

        let font_bytes = assets.get_font("JetBrainsMono-ExtraBold.ttf").expect("font");
        let font       = Font::from_bytes(&font_bytes).expect("invalid font");

        let mut scene    = Scene::new(context, CanvasMode::Fullscreen, 1);
        let layer_id     = LayerId(0);
        let (view_width, view_height) = scene.get_virtual_size();
        let canvas       = scene.get_layer_mut(layer_id).unwrap().canvas_mut();

        {
            let s = settings.get();
            add_rectangle(canvas, "bg",      0.0,                  0.0, view_width,                    view_height,     hex_to_color(&s.background),               "chrome");
            add_rectangle(canvas, "gutter",  0.0,                  0.0, s.gutter_width(),               view_height,     hex_to_color(&s.background_gutter),        "chrome");
            add_rectangle(canvas, "gut_sep", s.gutter_width()-1.0, 0.0, 1.0,                            view_height,     hex_to_color(&s.color_gutter_separator),   "chrome");
            add_rectangle(canvas, "row_hl",  s.gutter_width(),      0.0, view_width - s.gutter_width(), s.line_height(), hex_to_color(&s.background_row_highlight), "chrome");
            add_rectangle(canvas, "cursor",  s.text_start_x(),      0.0, s.cursor_width,                s.line_height(), hex_to_color(&s.color_cursor),             "chrome");
        }

        canvas.watch_source(SOURCE_FILE, settings.clone());

        let state_for_key    = state.clone();
        let settings_for_key = settings.clone();
        canvas.on_key_press(move |_canvas, key| {
            let tab_string        = settings_for_key.get().tab_string();
            let mut current_state = state_for_key.get_mut();
            match key {
                Key::Named(NamedKey::Enter)      => current_state.enter(),
                Key::Named(NamedKey::Delete)      => current_state.backspace(),
                Key::Named(NamedKey::ArrowLeft)  => current_state.move_left(),
                Key::Named(NamedKey::ArrowRight) => current_state.move_right(),
                Key::Named(NamedKey::ArrowUp)    => current_state.move_up(),
                Key::Named(NamedKey::ArrowDown)  => current_state.move_down(),
                Key::Named(NamedKey::Tab)        => current_state.insert_str(&tab_string),
                Key::Named(NamedKey::Space)      => current_state.insert_str(" "),
                Key::Character(characters) => {
                    if characters.as_str() == "\u{8}" || characters.as_str() == "\x7f" {
                        current_state.backspace();
                    } else if characters.chars().all(|c| !c.is_control()) {
                        current_state.insert_str(characters.as_str());
                    }
                }
                _ => {}
            }
        });

        let state_for_click    = state.clone();
        let settings_for_click = settings.clone();
        canvas.on_mouse_press(move |_canvas, _button, (click_x, click_y)| {
            state_for_click.get_mut().click(click_x, click_y, &settings_for_click.get());
        });

        let state_for_scroll    = state.clone();
        let settings_for_scroll = settings.clone();
        canvas.on_mouse_scroll(move |_canvas, (_delta_x, delta_y)| {
            let speed = settings_for_scroll.get().scroll_speed;
            let mut s = state_for_scroll.get_mut();
            s.scroll_by(delta_y * speed);
            s.start_render();
        });

        let font_for_tick     = font.clone();
        let settings_for_tick = settings.clone();
        canvas.on_update(move |canvas| {
            let view_width  = canvas.get_virtual_size().0;
            let view_height = canvas.get_virtual_size().1;

            let size_changed = {
                let s = state.get();
                (view_width  - s.last_view_width ).abs() > 0.5
                || (view_height - s.last_view_height).abs() > 0.5
            };
            if size_changed {
                let mut s         = state.get_mut();
                s.last_view_width  = view_width;
                s.last_view_height = view_height;
            }

            if settings_for_tick.changed() || size_changed {
                rebuild_chrome(canvas, &settings_for_tick.get(), view_width, view_height);
                state.get_mut().invalidate_all();
            }

            {
                let current_settings  = settings_for_tick.get();
                let mut current_state = state.get_mut();
                current_state.update_scroll_max(&current_settings, view_height);
                if current_state.snap_cursor {
                    current_state.ensure_cursor_visible(&current_settings, view_height);
                    current_state.snap_cursor = false;
                    current_state.start_render();
                }
            }

            // ── detect dirty flags ────────────────────────────────────────────
            let (scroll, cursor_row, cursor_column, content_dirty, scroll_dirty, cursor_dirty, edited_row) = {
                let mut s         = state.get_mut();
                let scroll        = s.scroll_y;
                let content_dirty = s.revision     != s.last_revision;
                let scroll_dirty  = (scroll - s.last_scroll).abs() > 0.01;
                let cursor_dirty  = s.cursor_row    != s.last_cursor_row
                                 || s.cursor_column != s.last_cursor_column;

                if content_dirty || scroll_dirty || cursor_dirty {
                    s.last_revision      = s.revision;
                    s.last_scroll        = scroll;
                    s.last_cursor_row    = s.cursor_row;
                    s.last_cursor_column = s.cursor_column;
                    // kick off a fresh render pass
                    s.render_slot   = 0;
                    s.pending_render = true;
                }

                let edited_row = s.last_edited_row.take();
                (scroll, s.cursor_row, s.cursor_column, content_dirty, scroll_dirty, cursor_dirty, edited_row)
            };

            // ── process one text slot this tick if work is pending ────────────
            if state.get().pending_render {
                let slot_count = state.get().line_names.len();
                text::update_text_slots(
                    canvas, &state, &settings_for_tick.get(), &font_for_tick,
                    scroll, cursor_row,
                    content_dirty, scroll_dirty, edited_row,
                );
                if state.get().render_slot >= slot_count {
                    state.get_mut().pending_render = false;
                }
            } else {
                return;
            }

            // ── cursor and row-highlight rects ────────────────────────────────
            let cursor_screen_y = cursor_row as f32 * settings_for_tick.get().line_height() - scroll;
            let is_visible      = cursor_screen_y > -settings_for_tick.get().line_height()
                                && cursor_screen_y < view_height;
            let draw_y          = if is_visible { cursor_screen_y } else { -settings_for_tick.get().line_height() * 2.0 };

            set_bounds(canvas, "row_hl",
                settings_for_tick.get().gutter_width(),
                draw_y,
                view_width - settings_for_tick.get().gutter_width(),
                settings_for_tick.get().line_height(),
            );

            let cursor_x = settings_for_tick.get().text_start_x()
                + cursor_column as f32 * settings_for_tick.get().char_width();
            set_bounds(canvas, "cursor",
                cursor_x,
                draw_y,
                settings_for_tick.get().cursor_width,
                settings_for_tick.get().line_height(),
            );
        });

        scene
    }
}

ramp::run! { |context: &mut Context, assets: Assets| {
    App::new(context, assets)
}}