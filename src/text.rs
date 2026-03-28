use flowmango::prelude::*;
use quartz::{Align, Color, Font, Shared, TextSpec, make_text_aligned};

use crate::{EditorSettings, State, hex_to_color};

pub fn create_text_spec(text: &str, font: &Font, color: Color, font_size: f32) -> TextSpec {
    make_text_aligned(text, font_size, font, color, Align::Left)
}

pub fn gutter_number_x(gutter_number: &str, settings: &EditorSettings) -> f32 {
    let number_width = gutter_number.len() as f32 * settings.char_width();
    (settings.gutter_width() - settings.number_padding_right - number_width)
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

pub fn update_text_slots(
    canvas:        &mut Canvas,
    state:         &Shared<State>,
    settings:      &EditorSettings,
    font:          &Font,
    scroll:        f32,
    cursor_row:    usize,
    content_dirty: bool,
    scroll_dirty:  bool,
    edited_row:    Option<usize>,
) {
    let new_first_row  = (scroll / settings.line_height()).floor() as usize;
    let subpixel_y     = scroll % settings.line_height();
    let text_y_padding = (settings.line_height() - settings.font_size) * 0.35;
    let total_lines    = state.get().lines.len();
    let slot_count     = state.get().line_names.len();

    let previous_first_row = state.get().first_row;
    if scroll_dirty && new_first_row != previous_first_row {
        let mut s = state.get_mut();
        s.cached_line_text.iter_mut().for_each(|t| t.clear());
        s.cached_gutter_number_text.iter_mut().for_each(|t| t.clear());
        s.cached_gutter_number_is_current.iter_mut().for_each(|f| *f = false);
        s.render_slot = 0;
    }
    state.get_mut().first_row = new_first_row;

    let slot_index = state.get().render_slot;
    if slot_index >= slot_count { return; }
    state.get_mut().render_slot += 1;

    let document_row = new_first_row + slot_index;
    let row_exists   = document_row < total_lines;
    let is_current   = document_row == cursor_row;

    let new_gutter_number = if row_exists {
        format!("{}", document_row + 1)
    } else {
        String::new()
    };

    let line_name   = state.get().line_names[slot_index].clone();
    let gutter_name = state.get().gutter_number_names[slot_index].clone();
    let screen_y    = slot_index as f32 * settings.line_height() - subpixel_y;
    let text_y      = screen_y + text_y_padding;

    // ── lazy-create line slot — track if just born ────────────────────────────
    let line_just_created = canvas.get_game_object(&line_name).is_none();
    if line_just_created {
        let spec = create_text_spec(
            "", font, hex_to_color(&settings.color_text), settings.font_size,
        );
        let mut object = GameObject::build(&line_name)
            .position(settings.text_start_x(), text_y)
            .size(4.0, settings.line_height())
            .tag("line")
            .finish();
        object.set_text(spec);
        canvas.add_game_object(line_name.clone(), object);
    }

    // ── lazy-create gutter slot — track if just born ──────────────────────────
    let gutter_just_created = canvas.get_game_object(&gutter_name).is_none();
    if gutter_just_created {
        let spec = create_text_spec(
            "", font, hex_to_color(&settings.color_line_number), settings.font_size,
        );
        let mut object = GameObject::build(&gutter_name)
            .position(settings.number_padding_left, text_y)
            .size(4.0, settings.line_height())
            .tag("gnum")
            .finish();
        object.set_text(spec);
        canvas.add_game_object(gutter_name.clone(), object);
    }

    // ── reposition on scroll ──────────────────────────────────────────────────
    if scroll_dirty {
        set_position(canvas, &line_name,   settings.text_start_x(), text_y);
        set_position(canvas, &gutter_name, gutter_number_x(&new_gutter_number, settings), text_y);
    }

    // ── update line text — also force on fresh slots ──────────────────────────
    let needs_text_rebuild = line_just_created || (content_dirty && match edited_row {
        Some(edited_row_index) => row_exists && document_row == edited_row_index,
        None => true,
    });
    if needs_text_rebuild {
        let new_text    = if row_exists { state.get().lines[document_row].clone() } else { String::new() };
        let cached_text = state.get().cached_line_text[slot_index].clone();
        if new_text != cached_text {
            let spec = create_text_spec(
                &new_text, font, hex_to_color(&settings.color_text), settings.font_size,
            );
            update_text_object(canvas, &line_name, spec);
            state.get_mut().cached_line_text[slot_index] = new_text;
        }
    }

    // ── reposition gutter number ──────────────────────────────────────────────
    let previous_gutter_len = state.get().cached_gutter_number_text[slot_index].len();
    if scroll_dirty || new_gutter_number.len() != previous_gutter_len {
        set_position(
            canvas,
            &gutter_name,
            gutter_number_x(&new_gutter_number, settings),
            text_y,
        );
    }

    // ── update gutter number text / color — also force on fresh slots ─────────
    let (cached_gutter_number, cached_is_current) = {
        let s = state.get();
        (
            s.cached_gutter_number_text[slot_index].clone(),
            s.cached_gutter_number_is_current[slot_index],
        )
    };
    if gutter_just_created || new_gutter_number != cached_gutter_number || is_current != cached_is_current {
        let color = if is_current {
            hex_to_color(&settings.color_line_number_active)
        } else {
            hex_to_color(&settings.color_line_number)
        };
        let spec = create_text_spec(&new_gutter_number, font, color, settings.font_size);
        update_text_object(canvas, &gutter_name, spec);
        let mut s = state.get_mut();
        s.cached_gutter_number_text[slot_index]       = new_gutter_number;
        s.cached_gutter_number_is_current[slot_index] = is_current;
    }
}



