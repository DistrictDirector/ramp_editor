use flowmango::prelude::*;
use quartz::{Align, Color, Font, Shared, TextSpec, SpanSpec, make_text_aligned};

use syntect::highlighting::{ThemeSet, Style};
use syntect::parsing::SyntaxSet;
use syntect::easy::HighlightLines;

use std::rc::Rc;

use crate::{EditorSettings, State, SlotCache, hex_to_color};

// ── SyntaxHighlighter ─────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct SyntaxHighlighter {
    syntax_set: SyntaxSet,
    theme_set:  Rc<ThemeSet>,
    theme_name: String,
}

impl SyntaxHighlighter {
    pub fn new() -> Self {
        let mut theme_set = ThemeSet::load_defaults();
        let cobalt = ThemeSet::get_theme("resources/cobalt.tmTheme").unwrap();
        theme_set.themes.insert("Cobalt".to_string(), cobalt);
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set:  Rc::new(theme_set),
            theme_name: "Cobalt".to_string(),
        }
    }

    pub fn highlight_line(
        &self, line_text: &str, font: &Font, font_size: f32, fallback: Color,
    ) -> TextSpec {
        if line_text.is_empty() {
            return make_text_aligned("", font_size, font, fallback, Align::Left);
        }

        let syntax = self.syntax_set
            .find_syntax_by_extension("rs")
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let theme = match self.theme_set.themes.get(&self.theme_name) {
            Some(t) => t,
            None    => return make_text_aligned(line_text, font_size, font, fallback, Align::Left),
        };

        let mut h     = HighlightLines::new(syntax, theme);
        let input     = if line_text.ends_with('\n') {
            line_text.to_string()
        } else {
            format!("{}\n", line_text)
        };

        let regions = match h.highlight_line(&input, &self.syntax_set) {
            Ok(r)  => r,
            Err(_) => return make_text_aligned(line_text, font_size, font, fallback, Align::Left),
        };

        let mut spans: Vec<SpanSpec> = Vec::with_capacity(regions.len());
        for (style, text) in &regions {
            let clean: String = text.chars().filter(|c| *c != '\n' && *c != '\r').collect();
            if clean.is_empty() { continue; }
            spans.push(SpanSpec {
                text:           clean,
                font_size,
                line_height:    None,
                font:           font.clone(),
                color:          syntect_color(style),
                letter_spacing: 0.0,
            });
        }

        if spans.is_empty() {
            return make_text_aligned(line_text, font_size, font, fallback, Align::Left);
        }
        TextSpec::new(spans, Align::Left)
    }
}

fn syntect_color(style: &Style) -> Color {
    Color(style.foreground.r, style.foreground.g, style.foreground.b, style.foreground.a)
}

// ── Public helpers ────────────────────────────────────────────────────────────

pub fn create_text_spec(text: &str, font: &Font, color: Color, font_size: f32) -> TextSpec {
    make_text_aligned(text, font_size, font, color, Align::Left)
}

fn gutter_x(digits: usize, s: &EditorSettings, line_count: usize) -> f32 {
    let nw = digits as f32 * s.char_width();
    (s.gutter_width_for(line_count) - s.number_padding_right - nw).max(s.number_padding_left)
}

fn digit_count(n: usize) -> usize {
    if n == 0 { 1 } else { (n as f32).log10().floor() as usize + 1 }
}

fn gutter_color(is_active: bool, s: &EditorSettings) -> Color {
    if is_active { hex_to_color(&s.color_line_number_active) }
    else         { hex_to_color(&s.color_line_number) }
}

// ── Slot index math ───────────────────────────────────────────────────────────

#[inline]
fn slot_for(doc_row: usize, slot_count: usize) -> usize {
    doc_row % slot_count
}

#[inline]
fn logical_of(phys: usize, first_row: usize, slot_count: usize) -> usize {
    (phys + slot_count - first_row % slot_count) % slot_count
}

// ── Canvas write helpers (never hold a State borrow) ─────────────────────────

fn write_text(canvas: &mut Canvas, name: &str, spec: TextSpec) {
    if let Some(o) = canvas.get_game_object_mut(name) { o.set_text(spec); }
}

fn write_pos(canvas: &mut Canvas, name: &str, x: f32, y: f32) {
    if let Some(o) = canvas.get_game_object_mut(name) { o.position = (x, y); }
}

fn write_bounds(canvas: &mut Canvas, name: &str, x: f32, y: f32, w: f32, h: f32) {
    if let Some(o) = canvas.get_game_object_mut(name) {
        o.position = (x, y);
        o.size     = (w, h);
    }
}

// ── flush ─────────────────────────────────────────────────────────────────────
// Rules:
//   1. Always snapshot what you need from state.get() into locals FIRST.
//   2. Drop the Ref (let it go out of scope) BEFORE calling state.get_mut().
//   3. Never hold a Ref and a RefMut at the same time.

pub fn flush(
    canvas:      &mut Canvas,
    state:       &Shared<State>,
    s:           &EditorSettings,
    font:        &Font,
    highlighter: &SyntaxHighlighter,
    vw:          f32,
    vh:          f32,
) {
    // ── Snapshot immutable state ──────────────────────────────────────────────
    // All reads from state happen here in one block so the Ref is dropped
    // before any get_mut() call below.
    let (
        scroll, cursor_row, cursor_col, total_lines, slot_count,
        old_first_row,
        needs_layout, needs_chrome_reposition, dirty_cursor_chrome,
        dirty_all_text,
        dirty_gutters_from,
        dirty_gutter_deactivate, dirty_gutter_activate,
    ) = {
        let st = state.get();
        (
            st.scroll_y,
            st.cursor_row,
            st.cursor_column,
            st.lines.len(),
            st.slot_count(),
            st.first_row,
            st.needs_layout,
            st.needs_chrome_reposition,
            st.dirty_cursor_chrome,
            st.dirty_all_text,
            st.dirty_gutters_from,
            st.dirty_gutter_deactivate,
            st.dirty_gutter_activate,
        )
        // Ref dropped here
    };

    let lh             = s.line_height();
    let gw             = s.gutter_width_for(total_lines);
    let new_first_row  = (scroll / lh).floor() as usize;
    let subpixel_y     = scroll % lh;
    let text_y_pad     = (lh - s.font_size) * 0.35;

    // Drain dirty_doc_rows without holding state borrow.
    let dirty_rows: Vec<usize> = {
        let mut st = state.get_mut();
        std::mem::take(&mut st.dirty_doc_rows)
    };

    // Update first_row now that we know it.
    state.get_mut().first_row = new_first_row;

    // ── 1. Scroll recycling ───────────────────────────────────────────────────
    if old_first_row != usize::MAX && new_first_row != old_first_row {
        let raw   = new_first_row as i64 - old_first_row as i64;
        let delta = raw.unsigned_abs() as usize;

        if delta >= slot_count {
            for phys in 0..slot_count {
                let logical = logical_of(phys, new_first_row, slot_count);
                let doc_row = new_first_row + logical;
                blit_slot(canvas, state, s, font, highlighter,
                          phys, doc_row, cursor_row, total_lines,
                          new_first_row, subpixel_y, text_y_pad);
            }
        } else {
            for i in 0..delta {
                let (phys, doc_row) = if raw > 0 {
                    ((old_first_row + i) % slot_count,
                     new_first_row + slot_count - delta + i)
                } else {
                    ((new_first_row + i) % slot_count,
                     new_first_row + i)
                };
                blit_slot(canvas, state, s, font, highlighter,
                          phys, doc_row, cursor_row, total_lines,
                          new_first_row, subpixel_y, text_y_pad);
            }
        }
    } else if old_first_row == usize::MAX {
        // Very first frame: populate every slot.
        for phys in 0..slot_count {
            let logical = logical_of(phys, new_first_row, slot_count);
            let doc_row = new_first_row + logical;
            blit_slot(canvas, state, s, font, highlighter,
                      phys, doc_row, cursor_row, total_lines,
                      new_first_row, subpixel_y, text_y_pad);
        }
    }

    // ── 2. Layout ─────────────────────────────────────────────────────────────
    if needs_layout {
        for phys in 0..slot_count {
            let logical    = logical_of(phys, new_first_row, slot_count);
            let doc_row    = new_first_row + logical;
            let row_exists = doc_row < total_lines;
            let screen_y   = logical as f32 * lh - subpixel_y;
            let text_y     = screen_y + text_y_pad;
            let digits     = if row_exists { digit_count(doc_row + 1) } else { 0 };

            write_pos(canvas, &format!("line_{}", phys), s.text_start_x_for(total_lines), text_y);
            write_pos(canvas, &format!("gnum_{}", phys), gutter_x(digits, s, total_lines), text_y);
        }
        state.get_mut().needs_layout = false;
    }

    // ── 3. Chrome reposition ──────────────────────────────────────────────────
    if needs_chrome_reposition {
        macro_rules! upd {
            ($n:expr, $x:expr, $y:expr, $w:expr, $h:expr) => {
                write_bounds(canvas, $n, $x, $y, $w, $h);
            };
        }
        upd!("bg",      0.0,      0.0, vw,            vh);
        upd!("gutter",  0.0,      0.0, gw,            vh);
        upd!("gut_sep", gw - 1.0, 0.0, 1.0,           vh);
        upd!("row_hl",  gw,       0.0, vw - gw,       lh);
        upd!("cursor",  gw,       0.0, s.cursor_width, lh);
        state.get_mut().needs_chrome_reposition = false;
    }

    // ── 4. Single-line text updates ───────────────────────────────────────────
    for doc_row in dirty_rows {
        if doc_row < new_first_row || doc_row >= new_first_row + slot_count { continue; }
        let phys = slot_for(doc_row, slot_count);

        // Snapshot text and cache — borrow dropped before write.
        let (new_text, cached_text) = {
            let st = state.get();
            let nt = st.lines[doc_row].clone();
            let ct = st.slots[phys].text.clone();
            (nt, ct)
            // Ref dropped
        };

        if new_text != cached_text {
            let fallback = hex_to_color(&s.color_text);
            let spec     = highlighter.highlight_line(&new_text, font, s.font_size, fallback);
            write_text(canvas, &format!("line_{}", phys), spec);
            // Now safe to mutate.
            let mut st        = state.get_mut();
            st.slots[phys].text    = new_text;
            st.slots[phys].doc_row = doc_row;
        }
    }

    // ── 5. Structural text updates (stale slots) ──────────────────────────────
    if !dirty_all_text {
        // Collect stale slots first so we don't hold the borrow during canvas writes.
        let stale: Vec<(usize, usize)> = {
            let st = state.get();
            (0..slot_count)
                .filter_map(|phys| {
                    if st.slots[phys].doc_row != usize::MAX { return None; }
                    let logical = logical_of(phys, new_first_row, slot_count);
                    let doc_row = new_first_row + logical;
                    if doc_row >= total_lines { return None; }
                    Some((phys, doc_row))
                })
                .collect()
            // Ref dropped
        };

        for (phys, doc_row) in stale {
            let new_text = state.get().lines[doc_row].clone();
            // Ref dropped before write.
            let fallback = hex_to_color(&s.color_text);
            let spec     = highlighter.highlight_line(&new_text, font, s.font_size, fallback);
            write_text(canvas, &format!("line_{}", phys), spec);
            let mut st        = state.get_mut();
            st.slots[phys].text    = new_text;
            st.slots[phys].doc_row = doc_row;
        }
    }

    // ── 6. Full text repaint ──────────────────────────────────────────────────
    if dirty_all_text {
        for phys in 0..slot_count {
            let logical    = logical_of(phys, new_first_row, slot_count);
            let doc_row    = new_first_row + logical;
            let row_exists = doc_row < total_lines;

            // Snapshot text before releasing borrow.
            let new_text = if row_exists { state.get().lines[doc_row].clone() } else { String::new() };
            // Ref dropped.

            let fallback = hex_to_color(&s.color_text);
            let spec     = highlighter.highlight_line(&new_text, font, s.font_size, fallback);
            write_text(canvas, &format!("line_{}", phys), spec);

            let mut st        = state.get_mut();
            st.slots[phys].text    = new_text;
            st.slots[phys].doc_row = if row_exists { doc_row } else { usize::MAX };
        }
        state.get_mut().dirty_all_text = false;
    }

    // ── 7. Gutter renumber ────────────────────────────────────────────────────
    if let Some(from_doc) = dirty_gutters_from {
        for phys in 0..slot_count {
            let logical    = logical_of(phys, new_first_row, slot_count);
            let doc_row    = new_first_row + logical;
            if doc_row < from_doc { continue; }
            let row_exists = doc_row < total_lines;
            let label      = if row_exists { format!("{}", doc_row + 1) } else { String::new() };
            let is_active  = doc_row == cursor_row;
            let color      = gutter_color(is_active, s);
            let spec       = create_text_spec(&label, font, color, s.font_size);
            write_text(canvas, &format!("gnum_{}", phys), spec);
            state.get_mut().slots[phys].is_active = is_active;
        }
        state.get_mut().dirty_gutters_from = None;
    }

    // ── 8. Active gutter highlight (cursor row change) ────────────────────────
    // Process deactivate then activate. Each reads then writes independently.
    for (doc_row_opt, target_active) in [
        (dirty_gutter_deactivate, false),
        (dirty_gutter_activate,   true),
    ] {
        let Some(doc_row) = doc_row_opt else { continue };
        if doc_row < new_first_row || doc_row >= new_first_row + slot_count { continue; }
        let phys = slot_for(doc_row, slot_count);

        // Check current cached state — drop borrow before write.
        let currently = { state.get().slots[phys].is_active };
        if currently == target_active { continue; }

        let row_exists = doc_row < total_lines;
        let label      = if row_exists { format!("{}", doc_row + 1) } else { String::new() };
        let color      = gutter_color(target_active, s);
        let spec       = create_text_spec(&label, font, color, s.font_size);
        write_text(canvas, &format!("gnum_{}", phys), spec);
        state.get_mut().slots[phys].is_active = target_active;
    }

    state.get_mut().dirty_gutter_deactivate = None;
    state.get_mut().dirty_gutter_activate   = None;

    // ── 9. Cursor chrome ──────────────────────────────────────────────────────
    if dirty_cursor_chrome {
        let cursor_y = cursor_row as f32 * lh - scroll;
        let visible  = cursor_y > -lh && cursor_y < vh;
        let draw_y   = if visible { cursor_y } else { -lh * 2.0 };

        write_bounds(canvas, "row_hl",
            gw, draw_y, vw - gw, lh);
        write_bounds(canvas, "cursor",
            gw + cursor_col as f32 * s.char_width(), draw_y, s.cursor_width, lh);

        state.get_mut().dirty_cursor_chrome = false;
    }
}

// ── blit_slot ─────────────────────────────────────────────────────────────────
// Highlight + position one slot. Called only from scroll recycling (step 1)
// where we know new content is required.
// Carefully snapshots, drops the Ref, then writes.

fn blit_slot(
    canvas:      &mut Canvas,
    state:       &Shared<State>,
    s:           &EditorSettings,
    font:        &Font,
    highlighter: &SyntaxHighlighter,
    phys:        usize,
    doc_row:     usize,
    cursor_row:  usize,
    total_lines: usize,
    first_row:   usize,
    subpixel_y:  f32,
    text_y_pad:  f32,
) {
    let slot_count = state.get().slot_count();
    let lh         = s.line_height();
    let logical    = logical_of(phys, first_row, slot_count);
    let screen_y   = logical as f32 * lh - subpixel_y;
    let text_y     = screen_y + text_y_pad;
    let row_exists = doc_row < total_lines;

    // Snapshot cached text — Ref dropped before any write.
    let (new_text, cached_text) = {
        let st = state.get();
        let nt = if row_exists { st.lines[doc_row].clone() } else { String::new() };
        let ct = st.slots[phys].text.clone();
        (nt, ct)
        // Ref dropped
    };

    if new_text != cached_text {
        let fallback = hex_to_color(&s.color_text);
        let spec     = highlighter.highlight_line(&new_text, font, s.font_size, fallback);
        write_text(canvas, &format!("line_{}", phys), spec);
        let mut st        = state.get_mut();
        st.slots[phys].text    = new_text;
        st.slots[phys].doc_row = if row_exists { doc_row } else { usize::MAX };
    }

    let is_active = doc_row == cursor_row;
    let row_exists_now = doc_row < { state.get().lines.len() };
    let digits    = if row_exists_now { digit_count(doc_row + 1) } else { 0 };
    let total_now = state.get().lines.len();
    let label     = if row_exists_now { format!("{}", doc_row + 1) } else { String::new() };
    let color     = gutter_color(is_active, s);
    let gspec     = create_text_spec(&label, font, color, s.font_size);

    write_text(canvas, &format!("gnum_{}", phys), gspec);
    write_pos(canvas,  &format!("line_{}", phys), s.text_start_x_for(total_now), text_y);
    write_pos(canvas,  &format!("gnum_{}", phys), gutter_x(digits, s, total_now), text_y);

    state.get_mut().slots[phys].is_active = is_active;
}