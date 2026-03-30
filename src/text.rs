use flowmango::prelude::*;
use quartz::{Align, Color, Font, Shared, TextSpec, make_text_aligned};

use crate::{EditorSettings, State, hex_to_color};

pub fn create_text_spec(text: &str, font: &Font, color: Color, font_size: f32) -> TextSpec {
    make_text_aligned(text, font_size, font, color, Align::Left)
}

pub fn gutter_number_x(gutter_number: &str, settings: &EditorSettings, line_count: usize) -> f32 {
    let number_width = gutter_number.len() as f32 * settings.char_width();
    (settings.gutter_width_for(line_count) - settings.number_padding_right - number_width)
        .max(settings.number_padding_left)
}

fn set_position(canvas: &mut Canvas, name: &str, x: f32, y: f32) {
    if let Some(object) = canvas.get_game_object_mut(name) {
        object.position = (x, y);
    }
}

pub fn update_text_object(canvas: &mut Canvas, name: &str, spec: TextSpec) {
    if let Some(object) = canvas.get_game_object_mut(name) {
        object.set_text(spec);
    }
}

#[inline]
fn doc_to_slot(doc_row: usize, slot_count: usize) -> usize {
    doc_row % slot_count
}

#[inline]
fn slot_logical(phys: usize, first_row: usize, slot_count: usize) -> usize {
    (phys + slot_count - first_row % slot_count) % slot_count
}

fn rebuild_slot(
    canvas:      &mut Canvas,
    state:       &Shared<State>,
    settings:    &EditorSettings,
    font:        &Font,
    slot_index:  usize,
    doc_row:     usize,
    cursor_row:  usize,
    total_lines: usize,
) {
    let row_exists    = doc_row < total_lines;
    let line_text     = if row_exists { state.get().lines[doc_row].clone() } else { String::new() };
    let gutter_number = if row_exists { format!("{}", doc_row + 1) } else { String::new() };
    let is_current    = doc_row == cursor_row;

    let text_spec = create_text_spec(&line_text, font, hex_to_color(&settings.color_text), settings.font_size);
    update_text_object(canvas, &format!("line_{}", slot_index), text_spec);
    state.get_mut().cached_line_text[slot_index] = line_text;

    let color = if is_current {
        hex_to_color(&settings.color_line_number_active)
    } else {
        hex_to_color(&settings.color_line_number)
    };
    let gutter_spec = create_text_spec(&gutter_number, font, color, settings.font_size);
    update_text_object(canvas, &format!("gnum_{}", slot_index), gutter_spec);
    state.get_mut().cached_gutter_number_is_current[slot_index] = is_current;
}

pub fn update_text_slots(
    canvas:        &mut Canvas,
    state:         &Shared<State>,
    settings:      &EditorSettings,
    font:          &Font,
    scroll:        f32,
    cursor_row:    usize,
    cursor_dirty:  bool,
    content_dirty: bool,
    scroll_dirty:  bool,
    edited_row:    Option<usize>,
) {
    let new_first_row  = (scroll / settings.line_height()).floor() as usize;
    let subpixel_y     = scroll % settings.line_height();
    let text_y_padding = (settings.line_height() - settings.font_size) * 0.35;
    let total_lines    = state.get().lines.len();
    let slot_count     = state.get().line_names.len();
    let old_first_row  = state.get().first_row;

    if scroll_dirty && new_first_row != old_first_row && old_first_row != usize::MAX {
        let raw_delta = new_first_row as i64 - old_first_row as i64;
        let delta     = raw_delta.unsigned_abs() as usize;

        if delta >= slot_count {
            for slot_index in 0..slot_count {
                let logical = slot_logical(slot_index, new_first_row, slot_count);
                let doc_row = new_first_row + logical;
                rebuild_slot(canvas, state, settings, font, slot_index, doc_row, cursor_row, total_lines);
            }
        } else {
            for i in 0..delta {
                let (slot_index, doc_row) = if raw_delta > 0 {
                    let p = (old_first_row + i) % slot_count;
                    let r = new_first_row + slot_count - delta + i;
                    (p, r)
                } else {
                    let p = (new_first_row + i) % slot_count;
                    let r = new_first_row + i;
                    (p, r)
                };
                rebuild_slot(canvas, state, settings, font, slot_index, doc_row, cursor_row, total_lines);
            }
        }
    }

    state.get_mut().first_row = new_first_row;

    if scroll_dirty || cursor_dirty {
        for slot_index in 0..slot_count {
            let logical    = slot_logical(slot_index, new_first_row, slot_count);
            let doc_row    = new_first_row + logical;
            let row_exists = doc_row < total_lines;
            let screen_y   = logical as f32 * settings.line_height() - subpixel_y;
            let text_y     = screen_y + text_y_padding;
            let gutter_number = if row_exists { format!("{}", doc_row + 1) } else { String::new() };

            set_position(canvas, &format!("line_{}", slot_index), settings.text_start_x_for(total_lines), text_y);
            set_position(canvas, &format!("gnum_{}", slot_index), gutter_number_x(&gutter_number, settings, total_lines), text_y);
        }
    }

    if cursor_dirty {
        let prev_active_slot = state.get().cached_gutter_number_is_current
            .iter()
            .position(|&a| a);

        let cursor_in_view  = cursor_row >= new_first_row
            && cursor_row < new_first_row + slot_count;
        let new_active_slot = cursor_in_view
            .then(|| doc_to_slot(cursor_row, slot_count));

        let mut to_update: Vec<(usize, bool)> = Vec::new();
        if let Some(prev) = prev_active_slot {
            if new_active_slot != Some(prev) {
                to_update.push((prev, false));
            }
        }
        if let Some(next) = new_active_slot {
            if Some(next) != prev_active_slot {
                to_update.push((next, true));
            }
        }

        for (slot_index, is_current) in to_update {
            let logical       = slot_logical(slot_index, new_first_row, slot_count);
            let doc_row       = new_first_row + logical;
            let row_exists    = doc_row < total_lines;
            let gutter_number = if row_exists { format!("{}", doc_row + 1) } else { String::new() };
            let color = if is_current {
                hex_to_color(&settings.color_line_number_active)
            } else {
                hex_to_color(&settings.color_line_number)
            };
            let spec = create_text_spec(&gutter_number, font, color, settings.font_size);
            update_text_object(canvas, &format!("gnum_{}", slot_index), spec);
            state.get_mut().cached_gutter_number_is_current[slot_index] = is_current;
        }
    }

    let is_structural = content_dirty && edited_row.is_none();
    if is_structural && !state.get().render_gutters_flushed {
        for slot_index in 0..slot_count {
            let logical    = slot_logical(slot_index, new_first_row, slot_count);
            let doc_row    = new_first_row + logical;
            let row_exists = doc_row < total_lines;
            let gutter_number = if row_exists { format!("{}", doc_row + 1) } else { String::new() };
            let is_current    = doc_row == cursor_row;
            let color = if is_current {
                hex_to_color(&settings.color_line_number_active)
            } else {
                hex_to_color(&settings.color_line_number)
            };
            let spec = create_text_spec(&gutter_number, font, color, settings.font_size);
            update_text_object(canvas, &format!("gnum_{}", slot_index), spec);
            state.get_mut().cached_gutter_number_is_current[slot_index] = is_current;
        }
        state.get_mut().render_gutters_flushed = true;
    }

    let first_slot = state.get().render_slot;
    if first_slot >= slot_count { return; }

    state.get_mut().render_slot = first_slot + 1;

    let slot_index = first_slot;
    let logical    = slot_logical(slot_index, new_first_row, slot_count);
    let doc_row    = new_first_row + logical;
    let row_exists = doc_row < total_lines;

    let needs_text_rebuild = content_dirty && match edited_row {
        Some(edited) => row_exists && doc_row == edited,
        None         => true,
    };
    if needs_text_rebuild {
        let new_text = if row_exists { state.get().lines[doc_row].clone() } else { String::new() };
        let cached   = state.get().cached_line_text[slot_index].clone();
        if new_text != cached {
            let spec = create_text_spec(&new_text, font, hex_to_color(&settings.color_text), settings.font_size);
            update_text_object(canvas, &format!("line_{}", slot_index), spec);
            state.get_mut().cached_line_text[slot_index] = new_text;
        }
    }
}