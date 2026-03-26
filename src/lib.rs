use flowmango::prelude::*;
use quartz::{Align, Color, Font, NamedKey, TextSpec, make_text_aligned};
use image::RgbaImage;
use std::rc::Rc;
use std::cell::RefCell;
use std::time::Instant;
use ramp::prism;

// ─── Settings ─────────────────────────────────────────────────────────────────

struct EditorSettings {
    font_size:    f32,
    line_height:  f32,
    char_width:   f32,

    num_pad_l:    f32,
    num_pad_r:    f32,
    gutter_cols:  f32,

    cursor_width: f32,

    scroll_speed: f32,

    tab_size:     usize,

    vis_rows:     usize,

    bg:           &'static str,
    bg_gutter:    &'static str,
    bg_row_hl:    &'static str,
    col_text:     &'static str,
    col_lnum:     &'static str,
    col_lnum_act: &'static str,
    col_cursor:   &'static str,
    col_gut_sep:  &'static str,
}

impl EditorSettings {
    fn line_h(&self)   -> f32 { self.font_size * self.line_height }
    fn char_w(&self)   -> f32 { self.font_size * self.char_width }
    fn gutter_w(&self) -> f32 { self.char_w() * self.gutter_cols + self.num_pad_l + self.num_pad_r }
    fn text_x(&self)   -> f32 { self.gutter_w() }
    fn tab_str(&self)  -> String { " ".repeat(self.tab_size) }
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            font_size:    16.0,
            line_height:  1.4,
            char_width:   0.6,

            num_pad_l:    4.0,
            num_pad_r:    18.0,
            gutter_cols:  4.0,

            cursor_width: 2.0,

            scroll_speed: 1.0,

            tab_size:     4,

            vis_rows:     60,

            bg:           "#1e1e2e",
            bg_gutter:    "#181825",
            bg_row_hl:    "#2a2a3d",
            col_text:     "#cdd6f4",
            col_lnum:     "#45475a",
            col_lnum_act: "#cba6f7",
            col_cursor:   "#cba6f7",
            col_gut_sep:  "#ffffff",
        }
    }
}

// ─── Palette helper ───────────────────────────────────────────────────────────

fn c(hex: &str) -> Color {
    let h = hex.trim_start_matches('#');
    let v = u32::from_str_radix(h, 16).unwrap_or(0);
    Color(((v >> 16) & 0xFF) as u8, ((v >> 8) & 0xFF) as u8, (v & 0xFF) as u8, 255)
}

// ─── Canvas helpers ───────────────────────────────────────────────────────────

fn solid_img(w: f32, h: f32, col: Color) -> Image {
    let mut img = RgbaImage::new(1, 1);
    img.pixels_mut().for_each(|px| *px = image::Rgba([col.0, col.1, col.2, col.3]));
    Image { shape: ShapeType::Rectangle(0.0, (w, h), 0.0), image: img.into(), color: None }
}

fn add_rect(cv: &mut Canvas, name: &str, x: f32, y: f32, w: f32, h: f32, col: Color, tag: &str) {
    let mut obj = GameObject::build(name).position(x, y).size(w, h).tag(tag).finish();
    obj.set_image(solid_img(w, h, col));
    cv.add_game_object(name.to_string(), obj);
}

fn add_text_obj(cv: &mut Canvas, name: &str, x: f32, y: f32, spec: TextSpec, line_h: f32, tag: &str) {
    let mut obj = GameObject::build(name)
        .position(x, y)
        .size(4.0, line_h)
        .tag(tag)
        .finish();
    obj.set_text(spec);
    cv.add_game_object(name.to_string(), obj);
}

fn update_text(cv: &mut Canvas, name: &str, spec: TextSpec) {
    if let Some(obj) = cv.get_game_object_mut(name) {
        obj.set_text(spec);
    }
}

fn set_bounds(cv: &mut Canvas, name: &str, x: f32, y: f32, w: f32, h: f32) {
    if let Some(obj) = cv.get_game_object_mut(name) {
        obj.size     = (w, h);
        obj.position = (x, y);
    }
}

fn set_pos(cv: &mut Canvas, name: &str, x: f32, y: f32) {
    if let Some(obj) = cv.get_game_object_mut(name) {
        obj.position = (x, y);
    }
}

fn text_spec(text: &str, font: &Font, col: Color, font_size: f32) -> TextSpec {
    make_text_aligned(text, font_size, font, col, Align::Left)
}

fn gnum_x(gnum: &str, s: &EditorSettings) -> f32 {
    let num_w = gnum.len() as f32 * s.char_w();
    (s.gutter_w() - s.num_pad_r - num_w).max(s.num_pad_l)
}

// ─── Perf counter ─────────────────────────────────────────────────────────────

struct PerfFrame {
    start:        Instant,
    set_pos_calls:    u32,
    update_text_calls: u32,
}

impl PerfFrame {
    fn new() -> Self {
        Self { start: Instant::now(), set_pos_calls: 0, update_text_calls: 0 }
    }
}

// ─── Slot ─────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct Slot {
    doc_row: Option<usize>,
    text:    String,
    gnum:    String,
    is_cur:  bool,
    sub_y:   f32,   // last rendered sub_y — skip set_pos if unchanged
}

impl Default for Slot {
    fn default() -> Self {
        Self { doc_row: None, text: String::new(), gnum: String::new(), is_cur: false, sub_y: f32::MAX }
    }
}

// ─── Editor state ─────────────────────────────────────────────────────────────

struct State {
    lines:           Vec<String>,
    cursor_row:      usize,
    cursor_col:      usize,
    scroll_y:        f32,
    scroll_max:      f32,
    first_row:       usize,
    revision:        u64,
    last_rev:        u64,
    last_scroll:     f32,
    last_cursor_row: usize,
    last_cursor_col: usize,
    slots:           Vec<Slot>,
    line_names:      Vec<String>,
    gnum_names:      Vec<String>,
    perf:            PerfFrame,
}

impl State {
    fn new(s: &EditorSettings) -> Self {
        let line_names = (0..s.vis_rows).map(|i| format!("line_{}", i)).collect();
        let gnum_names = (0..s.vis_rows).map(|i| format!("gnum_{}", i)).collect();
        Self {
            lines:           vec![String::new()],
            cursor_row:      0,
            cursor_col:      0,
            scroll_y:        0.0,
            scroll_max:      0.0,
            first_row:       usize::MAX,
            revision:        1,
            last_rev:        0,
            last_scroll:     f32::MAX,
            last_cursor_row: usize::MAX,
            last_cursor_col: usize::MAX,
            slots:           vec![Slot::default(); s.vis_rows],
            line_names,
            gnum_names,
            perf:            PerfFrame::new(),
        }
    }

    fn bump(&mut self) { self.revision = self.revision.wrapping_add(1); }

    fn update_scroll_max(&mut self, s: &EditorSettings, vh: f32) {
        let content_h = self.lines.len() as f32 * s.line_h();
        self.scroll_max = (content_h - vh).max(0.0);
        self.scroll_y   = self.scroll_y.clamp(0.0, self.scroll_max);
    }

    fn scroll_by(&mut self, delta: f32) {
        self.scroll_y = (self.scroll_y + delta).clamp(0.0, self.scroll_max);
    }

    fn click(&mut self, vx: f32, vy: f32, s: &EditorSettings) {
        if vx < s.gutter_w() { return; }
        let row   = ((vy + self.scroll_y) / s.line_h()).floor() as usize;
        let row   = row.min(self.lines.len().saturating_sub(1));
        let col_f = ((vx - s.text_x()) / s.char_w()).round();
        let col   = if col_f < 0.0 { 0 } else { col_f as usize };
        let col   = col.min(self.lines[row].chars().count());
        self.cursor_row = row;
        self.cursor_col = col;
        self.bump();
    }

    fn insert_str(&mut self, s: &str) {
        let row = self.cursor_row;
        let bi  = self.char_to_byte(row, self.cursor_col);
        self.lines[row].insert_str(bi, s);
        self.cursor_col += s.chars().count();
        self.bump();
    }

    fn backspace(&mut self) {
        if self.cursor_col > 0 {
            let row   = self.cursor_row;
            let start = self.char_to_byte(row, self.cursor_col - 1);
            let end   = self.char_to_byte(row, self.cursor_col);
            self.lines[row].drain(start..end);
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            let row      = self.cursor_row;
            let line     = self.lines.remove(row);
            let prev     = row - 1;
            let prev_len = self.lines[prev].chars().count();
            self.lines[prev].push_str(&line);
            self.cursor_row = prev;
            self.cursor_col = prev_len;
        }
        self.bump();
    }

    fn enter(&mut self) {
        let row  = self.cursor_row;
        let bi   = self.char_to_byte(row, self.cursor_col);
        let rest = self.lines[row].split_off(bi);
        self.cursor_row += 1;
        self.cursor_col  = 0;
        self.lines.insert(self.cursor_row, rest);
        self.bump();
    }

    fn move_left(&mut self) {
        if self.cursor_col > 0 { self.cursor_col -= 1; }
        else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col  = self.lines[self.cursor_row].chars().count();
        }
        self.bump();
    }

    fn move_right(&mut self) {
        let len = self.lines[self.cursor_row].chars().count();
        if self.cursor_col < len { self.cursor_col += 1; }
        else if self.cursor_row < self.lines.len() - 1 {
            self.cursor_row += 1; self.cursor_col = 0;
        }
        self.bump();
    }

    fn move_up(&mut self) {
        if self.cursor_row > 0 { self.cursor_row -= 1; self.clamp_col(); }
        self.bump();
    }

    fn move_down(&mut self) {
        if self.cursor_row < self.lines.len() - 1 {
            self.cursor_row += 1; self.clamp_col();
        }
        self.bump();
    }

    fn char_to_byte(&self, row: usize, ci: usize) -> usize {
        self.lines[row].char_indices().nth(ci).map(|(i, _)| i).unwrap_or(self.lines[row].len())
    }

    fn clamp_col(&mut self) {
        self.cursor_col = self.cursor_col.min(self.lines[self.cursor_row].chars().count());
    }
}

// ─── App ──────────────────────────────────────────────────────────────────────

pub struct App;

impl App {
    pub fn new(ctx: &mut Context, assets: Assets) -> Scene {
        let s = Rc::new(EditorSettings::default());

        let font_bytes = assets.get_font("JetBrainsMono-ExtraBold.ttf").expect("font");
        let font       = Font::from_bytes(&font_bytes).expect("invalid font");

        let mut scene = Scene::new(ctx, CanvasMode::Fullscreen, 1);
        let lid       = LayerId(0);
        let (vw, vh)  = scene.get_virtual_size();

        {
            let cv = scene.get_layer_mut(lid).unwrap().canvas_mut();

            add_rect(cv, "bg",      0.0,                0.0, vw,                vh,         c(s.bg),          "chrome");
            add_rect(cv, "gutter",  0.0,                0.0, s.gutter_w(),      vh,         c(s.bg_gutter),   "chrome");
            add_rect(cv, "gut_sep", s.gutter_w() - 1.0, 0.0, 1.0,               vh,         c(s.col_gut_sep), "chrome");
            add_rect(cv, "row_hl",  s.gutter_w(),       0.0, vw - s.gutter_w(), s.line_h(), c(s.bg_row_hl),   "chrome");
            add_rect(cv, "cursor",  s.text_x(),         0.0, s.cursor_width,    s.line_h(), c(s.col_cursor),  "chrome");

            for i in 0..s.vis_rows {
                let y = i as f32 * s.line_h();
                add_text_obj(cv, &format!("line_{}", i), s.text_x(), y,
                    text_spec("", &font, c(s.col_text), s.font_size), s.line_h(), "line");
                add_text_obj(cv, &format!("gnum_{}", i), s.num_pad_l, y,
                    text_spec("", &font, c(s.col_lnum), s.font_size), s.line_h(), "gnum");
            }


        }

        let state = Rc::new(RefCell::new(State::new(&s)));

        {
            let st_key = Rc::clone(&state);
            let s_key  = Rc::clone(&s);
            let cv     = scene.get_layer_mut(lid).unwrap().canvas_mut();

            cv.on_key_press(move |_cv, key| {
                let mut st = st_key.borrow_mut();
                match key {
                    Key::Named(NamedKey::Enter)      => st.enter(),
                    Key::Named(NamedKey::Delete)     => st.backspace(),
                    Key::Named(NamedKey::ArrowLeft)  => st.move_left(),
                    Key::Named(NamedKey::ArrowRight) => st.move_right(),
                    Key::Named(NamedKey::ArrowUp)    => st.move_up(),
                    Key::Named(NamedKey::ArrowDown)  => st.move_down(),
                    Key::Named(NamedKey::Tab)        => st.insert_str(&s_key.tab_str()),
                    Key::Named(NamedKey::Space)      => st.insert_str(" "),
                    Key::Character(s) => {
                        if s.as_str() == "\u{8}" || s.as_str() == "\x7f" {
                            st.backspace();
                        } else if s.chars().all(|ch| !ch.is_control()) {
                            st.insert_str(s.as_str());
                        }
                    }
                    _ => {}
                }
            });
        }

        {
            let st_click = Rc::clone(&state);
            let s_click  = Rc::clone(&s);
            let cv       = scene.get_layer_mut(lid).unwrap().canvas_mut();

            cv.on_mouse_press(move |_cv, _btn, (vx, vy)| {
                st_click.borrow_mut().click(vx, vy, &s_click);
            });
        }

        {
            let st_scroll = Rc::clone(&state);
            let s_scroll  = Rc::clone(&s);
            let cv        = scene.get_layer_mut(lid).unwrap().canvas_mut();

            cv.on_mouse_scroll(move |_cv, (_dx, dy)| {
                st_scroll.borrow_mut().scroll_by(dy * s_scroll.scroll_speed);
            });
        }

        {
            let font_tick = font.clone();
            let s_tick    = Rc::clone(&s);
            let cv        = scene.get_layer_mut(lid).unwrap().canvas_mut();

            cv.on_update(move |cv| {
                let s = &*s_tick;
                let (vw, vh) = cv.get_virtual_size();

                // Reset per-frame perf counters
                let frame_start = Instant::now();
                let mut set_pos_calls:     u32 = 0;
                let mut update_text_calls: u32 = 0;

                let (scroll, cursor_row, cursor_col, content_dirty, scroll_changed, cursor_moved) = {
                    let mut st = state.borrow_mut();
                    st.update_scroll_max(s, vh);

                    let scroll         = st.scroll_y;
                    let scroll_changed = (scroll - st.last_scroll).abs() > 0.01;
                    let content_dirty  = st.revision != st.last_rev;
                    let cursor_moved   = st.cursor_row != st.last_cursor_row
                                      || st.cursor_col != st.last_cursor_col;

                    if content_dirty || scroll_changed || cursor_moved {
                        st.last_rev        = st.revision;
                        st.last_scroll     = scroll;
                        st.last_cursor_row = st.cursor_row;
                        st.last_cursor_col = st.cursor_col;
                    }

                    (scroll, st.cursor_row, st.cursor_col, content_dirty, scroll_changed, cursor_moved)
                };

                if !content_dirty && !scroll_changed && !cursor_moved { return; }

                let new_first  = (scroll / s.line_h()).floor() as usize;
                let sub_y      = scroll % s.line_h();
                let text_y_pad = (s.line_h() - s.font_size) * 0.35;
                let total      = state.borrow().lines.len();
                let old_first  = state.borrow().first_row;

                // ── POSITION ─────────────────────────────────────────────────
                // sub_y is global — one check covers all 60 slots.
                let last_sub_y = state.borrow().slots[0].sub_y;
                if (scroll_changed || content_dirty) && (last_sub_y - sub_y).abs() > 0.01 {
                    for slot_i in 0..s.vis_rows {
                        let screen_y  = slot_i as f32 * s.line_h() - sub_y;
                        let text_y    = screen_y + text_y_pad;
                        let gnum_str  = state.borrow().slots[slot_i].gnum.clone();
                        let line_name = state.borrow().line_names[slot_i].clone();
                        let gnum_name = state.borrow().gnum_names[slot_i].clone();
                        set_pos(cv, &line_name, s.text_x(), text_y);
                        set_pos(cv, &gnum_name, gnum_x(&gnum_str, s), text_y);
                        set_pos_calls += 2;
                        state.borrow_mut().slots[slot_i].sub_y = sub_y;
                    }
                }

                // ── TEXT ─────────────────────────────────────────────────────
                // Each update_text costs ~1ms (text layout). Cap at 8 per frame
                // so scroll never blocks longer than ~8ms. Remaining dirty slots
                // update on the next frame.
                const MAX_TEXT_PER_FRAME: usize = 8;
                let mut text_budget = MAX_TEXT_PER_FRAME;

                if new_first != old_first || content_dirty || cursor_moved {
                    state.borrow_mut().first_row = new_first;

                    for slot_i in 0..s.vis_rows {
                        if text_budget == 0 { break; }

                        let doc_row = new_first + slot_i;
                        let exists  = doc_row < total;
                        let is_cur  = doc_row == cursor_row;

                        let (prev_doc, prev_cur) = {
                            let st = state.borrow();
                            (st.slots[slot_i].doc_row, st.slots[slot_i].is_cur)
                        };

                        let reassigned = prev_doc != if exists { Some(doc_row) } else { None };

                        if reassigned || content_dirty {
                            let new_text = if exists { state.borrow().lines[doc_row].clone() } else { String::new() };
                            let new_gnum = if exists { format!("{}", doc_row + 1) } else { String::new() };
                            let (prev_text, prev_gnum) = {
                                let st = state.borrow();
                                (st.slots[slot_i].text.clone(), st.slots[slot_i].gnum.clone())
                            };

                            if new_text != prev_text && text_budget > 0 {
                                let name = state.borrow().line_names[slot_i].clone();
                                update_text(cv, &name,
                                    text_spec(&new_text, &font_tick, c(s.col_text), s.font_size));
                                update_text_calls += 1;
                                text_budget -= 1;
                            }
                            if (new_gnum != prev_gnum || is_cur != prev_cur) && text_budget > 0 {
                                let col  = if is_cur { c(s.col_lnum_act) } else { c(s.col_lnum) };
                                let name = state.borrow().gnum_names[slot_i].clone();
                                update_text(cv, &name,
                                    text_spec(&new_gnum, &font_tick, col, s.font_size));
                                update_text_calls += 1;
                                text_budget -= 1;
                            }

                            let mut st = state.borrow_mut();
                            st.slots[slot_i].doc_row = if exists { Some(doc_row) } else { None };
                            st.slots[slot_i].text    = new_text;
                            st.slots[slot_i].gnum    = new_gnum;
                            st.slots[slot_i].is_cur  = is_cur;
                            st.slots[slot_i].sub_y   = f32::MAX;

                        } else if cursor_moved && is_cur != prev_cur && text_budget > 0 {
                            let gnum_str = state.borrow().slots[slot_i].gnum.clone();
                            let name     = state.borrow().gnum_names[slot_i].clone();
                            let col      = if is_cur { c(s.col_lnum_act) } else { c(s.col_lnum) };
                            update_text(cv, &name,
                                text_spec(&gnum_str, &font_tick, col, s.font_size));
                            update_text_calls += 1;
                            text_budget -= 1;
                            state.borrow_mut().slots[slot_i].is_cur = is_cur;
                        }
                    }
                }

                // ── Cursor and row highlight ──────────────────────────────────
                let cur_screen_y = cursor_row as f32 * s.line_h() - scroll;
                set_bounds(cv, "row_hl", s.gutter_w(), cur_screen_y, vw - s.gutter_w(), s.line_h());
                let cur_x = s.text_x() + cursor_col as f32 * s.char_w();
                set_bounds(cv, "cursor", cur_x, cur_screen_y, s.cursor_width, s.line_h());

                // ── Perf print ────────────────────────────────────────────────
                let elapsed_us = frame_start.elapsed().as_micros();
                println!(
                    "{:.2}ms | set_pos:{} update_text:{}",
                    elapsed_us as f32 / 1000.0,
                    set_pos_calls,
                    update_text_calls,
                );
            });
        }

        scene
    }
}

ramp::run! { |ctx: &mut Context, assets: Assets| {
    App::new(ctx, assets)
}}