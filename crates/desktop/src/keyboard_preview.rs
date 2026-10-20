//! Scaled laptop keyboard with function row, full legends, arrows and numpad.
use crate::lighting::Lighting;
use gtk::{cairo, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
const WIDTH: f64 = 19.5;
const HEIGHT: f64 = 6.75;
struct Key {
    label: &'static str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}
fn row(keys: &mut Vec<Key>, y: f64, labels: &[(&'static str, f64)]) {
    let mut x = 0.0;
    for &(label, width) in labels {
        keys.push(Key {
            label,
            x,
            y,
            width,
            height: 1.0,
        });
        x += width;
    }
}
fn layout() -> Vec<Key> {
    let mut keys = Vec::with_capacity(110);
    for (index, label) in [
        "Esc", "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "PrtSc",
        "Del", "Home", "End",
    ]
    .into_iter()
    .enumerate()
    {
        let x = if index == 0 {
            0.0
        } else {
            1.45 + (index - 1) as f64 * 1.12
        };
        keys.push(Key {
            label,
            x,
            y: 0.0,
            width: 1.0,
            height: 0.8,
        });
    }
    row(
        &mut keys,
        1.1,
        &[
            ("`", 1.),
            ("1", 1.),
            ("2", 1.),
            ("3", 1.),
            ("4", 1.),
            ("5", 1.),
            ("6", 1.),
            ("7", 1.),
            ("8", 1.),
            ("9", 1.),
            ("0", 1.),
            ("-", 1.),
            ("=", 1.),
            ("Backspace", 2.),
        ],
    );
    row(
        &mut keys,
        2.1,
        &[
            ("Tab", 1.5),
            ("Q", 1.),
            ("W", 1.),
            ("E", 1.),
            ("R", 1.),
            ("T", 1.),
            ("Y", 1.),
            ("U", 1.),
            ("I", 1.),
            ("O", 1.),
            ("P", 1.),
            ("[", 1.),
            ("]", 1.),
            ("\\", 1.5),
        ],
    );
    row(
        &mut keys,
        3.1,
        &[
            ("Caps", 1.75),
            ("A", 1.),
            ("S", 1.),
            ("D", 1.),
            ("F", 1.),
            ("G", 1.),
            ("H", 1.),
            ("J", 1.),
            ("K", 1.),
            ("L", 1.),
            (";", 1.),
            ("'", 1.),
            ("Enter", 2.25),
        ],
    );
    row(
        &mut keys,
        4.1,
        &[
            ("Shift", 2.25),
            ("Z", 1.),
            ("X", 1.),
            ("C", 1.),
            ("V", 1.),
            ("B", 1.),
            ("N", 1.),
            ("M", 1.),
            (",", 1.),
            (".", 1.),
            ("/", 1.),
            ("Shift", 1.75),
            ("↑", 1.),
        ],
    );
    row(
        &mut keys,
        5.1,
        &[
            ("Ctrl", 1.25),
            ("Fn", 1.),
            ("Win", 1.),
            ("Alt", 1.25),
            ("Space", 5.),
            ("Alt", 1.25),
            ("Ctrl", 1.25),
            ("←", 1.),
            ("↓", 1.),
            ("→", 1.),
        ],
    );
    for (y, labels) in [
        (1.1, ["Num", "/", "*", "−"]),
        (2.1, ["7", "8", "9", "+"]),
        (3.1, ["4", "5", "6", ""]),
        (4.1, ["1", "2", "3", "Enter"]),
        (5.1, ["0", "", ".", ""]),
    ] {
        for (column, label) in labels.into_iter().enumerate() {
            if label.is_empty() {
                continue;
            }
            keys.push(Key {
                label,
                x: 15.5 + column as f64,
                y,
                width: if label == "0" { 2.0 } else { 1.0 },
                height: if label == "+" || label == "Enter" {
                    2.0
                } else {
                    1.0
                },
            });
        }
    }
    keys
}
fn rounded(context: &cairo::Context, x: f64, y: f64, width: f64, height: f64, radius: f64) {
    context.new_sub_path();
    for (cx, cy, angle) in [
        (x + width - radius, y + radius, -90.0_f64),
        (x + width - radius, y + height - radius, 0.0),
        (x + radius, y + height - radius, 90.0),
        (x + radius, y + radius, 180.0),
    ] {
        context.arc(
            cx,
            cy,
            radius,
            angle.to_radians(),
            (angle + 90.0).to_radians(),
        );
    }
    context.close_path();
}
fn transform(width: i32, height: i32) -> (f64, f64, f64) {
    let scale = ((width as f64 - 32.0) / WIDTH)
        .min((height as f64 - 24.0) / HEIGHT)
        .max(0.1);
    (
        scale,
        (width as f64 - WIDTH * scale) / 2.0,
        (height as f64 - HEIGHT * scale) / 2.0,
    )
}
pub fn new(state: Rc<RefCell<Lighting>>, target: &gtk::DropDown) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder()
        .content_height(275)
        .hexpand(true)
        .build();
    area.set_tooltip_text(Some("Click a key to select its lighting zone"));
    let keys = layout();
    let elapsed = Rc::new(Cell::new(0.0));
    let frame_time = elapsed.clone();
    let settings = state.clone();
    let first = Cell::new(None::<i64>);
    let last = Cell::new(0i64);
    area.add_tick_callback(move |area, clock| {
        let now = clock.frame_time();
        let start = first.get().unwrap_or(now);
        first.set(Some(start));
        if settings.borrow().animated() && area.is_mapped() && now - last.get() >= 41_666 {
            frame_time.set((now - start) as f64 / 1_000_000.0);
            last.set(now);
            area.queue_draw();
        }
        glib::ControlFlow::Continue
    });
    let settings = state;
    area.set_draw_func(move |_, context, width, height| {
        let state = settings.borrow();
        let (scale, ox, oy) = transform(width, height);
        let _ = context.save();
        context.translate(ox, oy);
        context.scale(scale, scale);
        for (index, key) in keys.iter().enumerate() {
            let [red, green, blue] =
                state.color(key.x + key.width / 2.0, key.y, index, elapsed.get());
            let x = key.x + 0.05;
            let y = key.y + 0.05;
            let w = key.width - 0.1;
            let h = key.height - 0.1;
            rounded(context, x, y + 0.06, w, h, 0.12);
            context.set_source_rgb(0.025, 0.025, 0.025);
            let _ = context.fill();
            rounded(context, x, y, w, h - 0.03, 0.12);
            context.set_source_rgb(
                0.075 + red * 0.16,
                0.075 + green * 0.16,
                0.075 + blue * 0.16,
            );
            let _ = context.fill_preserve();
            let zone = ((key.x / WIDTH * 4.0) as usize).min(3) + 1;
            let selected = state.zones && state.selected == zone;
            context.set_source_rgba(red, green, blue, if selected { 1.0 } else { 0.65 });
            context.set_line_width(if selected { 0.045 } else { 0.02 });
            let _ = context.stroke();
            context.set_source_rgba(red, green, blue, 0.9);
            context.rectangle(x + 0.14, y + h - 0.12, (w - 0.28).max(0.1), 0.045);
            let _ = context.fill();
            context.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
            context.set_font_size(if key.label.len() > 5 { 0.20 } else { 0.25 });
            context.set_source_rgb(0.88 + red * 0.12, 0.88 + green * 0.12, 0.88 + blue * 0.12);
            if let Some(angle) = match key.label {
                "↑" => Some(-90.0_f64),
                "↓" => Some(90.0),
                "←" => Some(180.0),
                "→" => Some(0.0),
                _ => None,
            } {
                let _ = context.save();
                context.translate(x + w / 2.0, y + h / 2.0);
                context.rotate(angle.to_radians());
                context.set_line_width(0.035);
                context.move_to(-0.13, 0.0);
                context.line_to(0.13, 0.0);
                context.move_to(0.02, -0.11);
                context.line_to(0.13, 0.0);
                context.line_to(0.02, 0.11);
                let _ = context.stroke();
                let _ = context.restore();
            } else if let Ok(text) = context.text_extents(key.label) {
                context.move_to(
                    x + (w - text.width()) / 2.0 - text.x_bearing(),
                    y + h / 2.0 - text.y_bearing() / 2.0,
                );
                let _ = context.show_text(key.label);
            }
        }
        for zone in 0..4 {
            let color = state.colors[zone];
            let x = zone as f64 * WIDTH / 4.0;
            context.set_source_rgba(color[0], color[1], color[2], 0.6);
            context.rectangle(x, 6.35, WIDTH / 4.0 - 0.12, 0.04);
            let _ = context.fill();
            context.set_source_rgb(0.6, 0.6, 0.6);
            context.set_font_size(0.22);
            context.move_to(x + 0.1, 6.7);
            let _ = context.show_text(&format!("ZONE {}", zone + 1));
        }
        let _ = context.restore();
    });
    let click = gtk::GestureClick::new();
    let weak = area.downgrade();
    let selector = target.clone();
    click.connect_pressed(move |_, _, x, _| {
        if let Some(area) = weak.upgrade() {
            let (scale, ox, _) = transform(area.width(), area.height());
            let zone = (((x - ox) / scale / WIDTH * 4.0).clamp(0.0, 3.0) as u32) + 1;
            selector.set_selected(zone);
        }
    });
    area.add_controller(click);
    area
}
