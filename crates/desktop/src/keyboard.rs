//! Four-zone lighting editor. Drafts and animations remain local until Apply.
use crate::{
    client::{self, Client},
    keyboard_preview,
    lighting::{self, Lighting},
};
use gtk::prelude::*;
use predator_core::Status;
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, PartialEq)]
struct LightingSnapshot {
    effect: Option<String>,
    zones: Option<String>,
    per_zone: bool,
}
#[derive(Clone)]
pub struct Keyboard {
    pub widget: gtk::Box,
    kind: gtk::DropDown,
    mode: gtk::DropDown,
    speed: gtk::SpinButton,
    brightness: gtk::Scale,
    direction: gtk::DropDown,
    colors: Vec<gtk::Entry>,
    apply: gtk::Button,
    previous: RefCell<Option<LightingSnapshot>>,
}
fn note(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.add_css_class("muted");
    label
}
fn redraw(preview: &gtk::glib::WeakRef<gtk::DrawingArea>) {
    if let Some(preview) = preview.upgrade() {
        preview.queue_draw();
    }
}
impl Keyboard {
    pub fn new(client: &Client) -> Self {
        let widget = client::page(
            "Keyboard lighting",
            "Choose a mode, preview your colours, then apply them to your keyboard.",
        );
        let state = Rc::new(RefCell::new(Lighting::default()));
        let target =
            gtk::DropDown::from_strings(&["All zones", "Zone 1", "Zone 2", "Zone 3", "Zone 4"]);
        let preview = keyboard_preview::new(state.clone(), &target);
        let kind = gtk::DropDown::from_strings(&["RGB effects", "Four-zone colours"]);
        let mode = gtk::DropDown::from_strings(&lighting::EFFECTS);
        let apply = gtk::Button::with_label("Apply lighting");
        apply.add_css_class("suggested-action");
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        kind.set_hexpand(true);
        mode.set_hexpand(true);
        let kind_column = gtk::Box::new(gtk::Orientation::Vertical, 6);
        kind_column.set_hexpand(true);
        kind_column.append(&note("LIGHTING TYPE"));
        kind_column.append(&kind);
        let mode_column = gtk::Box::new(gtk::Orientation::Vertical, 6);
        mode_column.set_hexpand(true);
        mode_column.append(&note("RGB MODE"));
        mode_column.append(&mode);
        apply.set_valign(gtk::Align::End);
        toolbar.append(&kind_column);
        toolbar.append(&mode_column);
        toolbar.append(&apply);
        widget.append(&toolbar);
        let hero = client::card(
            "Live preview",
            "Click a key to select its zone. The PHN16-71 controls four lighting zones.",
        );
        hero.append(&preview);
        let preview_bar = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let hint = note("Animations are an approximation of the firmware effect.");
        hint.set_hexpand(true);
        let play = gtk::ToggleButton::with_label("Pause preview");
        play.set_active(true);
        preview_bar.append(&hint);
        preview_bar.append(&play);
        hero.append(&preview_bar);
        widget.append(&hero);
        let settings = client::card(
            "Effect settings",
            "Brightness applies to both effects and four-zone colours.",
        );
        let brightness = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
        brightness.set_value(100.0);
        brightness.set_draw_value(true);
        brightness.set_hexpand(true);
        client::labeled(&settings, "Brightness", &brightness);
        let effect_options = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        let speed = client::spin(0.0, 9.0, 3.0);
        let direction = gtk::DropDown::from_strings(&["Left to right", "Right to left"]);
        client::labeled(&effect_options, "Speed", &speed);
        client::labeled(&effect_options, "Direction", &direction);
        settings.append(&effect_options);
        widget.append(&settings);
        let palette = client::card(
            "Colour palette",
            "Pick a swatch, open a colour picker, or enter a six-digit HEX colour.",
        );
        client::labeled(&palette, "Apply palette colour to", &target);
        let swatches = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        palette.append(&swatches);
        let color_grid = gtk::Grid::builder()
            .column_spacing(12)
            .row_spacing(12)
            .column_homogeneous(true)
            .build();
        let mut colors = Vec::new();
        for index in 0..4 {
            let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
            column.append(&note(&format!("ZONE {}", index + 1)));
            let dialog = gtk::ColorDialog::builder()
                .title(format!("Choose zone {} colour", index + 1))
                .with_alpha(false)
                .build();
            let picker = gtk::ColorDialogButton::new(Some(dialog));
            picker.set_hexpand(true);
            picker.set_height_request(40);
            picker.set_tooltip_text(Some("Open colour palette and RGB picker"));
            picker.set_rgba(&gtk::gdk::RGBA::new(0.27, 0.87, 0.8, 1.0));
            let entry = gtk::Entry::builder()
                .text("44ddcc")
                .max_length(7)
                .width_chars(8)
                .hexpand(true)
                .build();
            column.append(&picker);
            column.append(&entry);
            color_grid.attach(&column, index, 0, 1, 1);
            let weak = entry.downgrade();
            picker.connect_rgba_notify(move |picker| {
                if let Some(entry) = weak.upgrade() {
                    let color = picker.rgba();
                    let value = lighting::hex([
                        color.red() as f64,
                        color.green() as f64,
                        color.blue() as f64,
                    ]);
                    if entry.text().trim_start_matches('#').to_lowercase() != value {
                        entry.set_text(&value);
                    }
                }
            });
            let weak_picker = picker.downgrade();
            let preview = preview.downgrade();
            let state = state.clone();
            entry.connect_changed(move |entry| {
                if let Some(color) = lighting::parse_color(&entry.text()) {
                    entry.remove_css_class("error");
                    state.borrow_mut().colors[index as usize] = color;
                    if let Some(picker) = weak_picker.upgrade() {
                        let rgba = gtk::gdk::RGBA::new(
                            color[0] as f32,
                            color[1] as f32,
                            color[2] as f32,
                            1.0,
                        );
                        if picker.rgba() != rgba {
                            picker.set_rgba(&rgba);
                        }
                    }
                    redraw(&preview);
                } else {
                    entry.add_css_class("error");
                }
            });
            colors.push(entry);
        }
        palette.append(&color_grid);
        let bottom_apply = gtk::Button::with_label("Apply lighting");
        bottom_apply.add_css_class("suggested-action");
        let weak_apply = apply.downgrade();
        bottom_apply.connect_clicked(move |_| {
            if let Some(apply) = weak_apply.upgrade() {
                apply.emit_clicked();
            }
        });
        palette.append(&bottom_apply);
        let capability =
            note("Static colour uses Zone 1. Neon and Rainbow wave generate their own spectrum.");
        palette.append(&capability);
        widget.append(&palette);
        for value in [
            "ff4055", "ff9f43", "ffe66d", "53e58c", "43d9e8", "729dff", "bd8cff", "ff88cc",
            "ffffff",
        ] {
            let button = gtk::Button::new();
            button.add_css_class("palette-swatch");
            button.set_tooltip_text(Some(&format!("#{value}")));
            let swatch = gtk::DrawingArea::builder()
                .content_width(28)
                .content_height(28)
                .build();
            let rgb = lighting::parse_color(value).unwrap();
            swatch.set_draw_func(move |_, context, width, height| {
                context.set_source_rgb(rgb[0], rgb[1], rgb[2]);
                context.rectangle(0.0, 0.0, width as f64, height as f64);
                let _ = context.fill();
            });
            button.set_child(Some(&swatch));
            let entries = colors.clone();
            let selector = target.downgrade();
            let kind = kind.downgrade();
            button.connect_clicked(move |_| {
                let per_zone = kind.upgrade().is_some_and(|kind| kind.selected() == 1);
                let selected = selector.upgrade().map_or(0, |selector| selector.selected());
                for (index, entry) in entries.iter().enumerate() {
                    if (!per_zone && index == 0)
                        || (per_zone && (selected == 0 || selected == index as u32 + 1))
                    {
                        entry.set_text(value);
                    }
                }
            });
            swatches.append(&button);
        }
        // Signal handlers keep only weak widget references; the preview state
        // contains values, so palette/editor cross-updates cannot form a cycle.
        let weak_preview = preview.downgrade();
        let values = state.clone();
        brightness.connect_value_changed(move |control| {
            values.borrow_mut().brightness = control.value() / 100.0;
            redraw(&weak_preview);
        });
        let weak_preview = preview.downgrade();
        let values = state.clone();
        speed.connect_value_changed(move |control| {
            values.borrow_mut().speed = control.value();
            redraw(&weak_preview);
        });
        let weak_preview = preview.downgrade();
        let values = state.clone();
        direction.connect_selected_notify(move |control| {
            values.borrow_mut().reverse = control.selected() == 1;
            redraw(&weak_preview);
        });
        let weak_preview = preview.downgrade();
        let values = state.clone();
        target.connect_selected_notify(move |control| {
            values.borrow_mut().selected = control.selected() as usize;
            redraw(&weak_preview);
        });
        let weak_preview = preview.downgrade();
        let values = state.clone();
        play.connect_toggled(move |control| {
            values.borrow_mut().playing = control.is_active();
            control.set_label(if control.is_active() {
                "Pause preview"
            } else {
                "Play preview"
            });
            redraw(&weak_preview);
        });
        let weak_preview = preview.downgrade();
        let values = state.clone();
        let weak_speed = speed.downgrade();
        let weak_direction = direction.downgrade();
        let weak_palette = palette.downgrade();
        mode.connect_selected_notify(move |control| {
            let effect = control.selected();
            values.borrow_mut().effect = effect;
            if let Some(speed) = weak_speed.upgrade() {
                speed.set_sensitive(effect >= 2);
            }
            if let Some(direction) = weak_direction.upgrade() {
                direction.set_sensitive(matches!(effect, 3 | 4));
                values.borrow_mut().reverse = matches!(effect, 3 | 4) && direction.selected() == 1;
            }
            if let Some(palette) = weak_palette.upgrade() {
                palette.set_sensitive(!matches!(effect, 2 | 3));
            }
            redraw(&weak_preview);
        });
        let weak_preview = preview.downgrade();
        let values = state;
        let weak_mode = mode.downgrade();
        let weak_options = effect_options.downgrade();
        let weak_palette = palette.downgrade();
        let weak_target = target.downgrade();
        kind.connect_selected_notify(move |control| {
            let zones = control.selected() == 1;
            values.borrow_mut().zones = zones;
            if let Some(mode) = weak_mode.upgrade() {
                mode.set_sensitive(!zones);
            }
            if let Some(options) = weak_options.upgrade() {
                options.set_visible(!zones);
            }
            if let Some(target) = weak_target.upgrade() {
                target.set_sensitive(zones);
            }
            if let Some(palette) = weak_palette.upgrade() {
                palette.set_sensitive(zones || !matches!(values.borrow().effect, 2 | 3));
            }
            redraw(&weak_preview);
        });
        speed.set_sensitive(false);
        direction.set_sensitive(false);
        target.set_sensitive(false);
        let selected_kind = kind.clone();
        let selected_mode = mode.clone();
        let selected_speed = speed.clone();
        let selected_direction = direction.clone();
        let selected_brightness = brightness.clone();
        let entries = colors.clone();
        let connection = client.clone();
        apply.connect_clicked(move |_| {
            let Some(parsed) = entries
                .iter()
                .map(|entry| lighting::parse_color(&entry.text()))
                .collect::<Option<Vec<_>>>()
            else {
                connection.error("Choose valid six-digit HEX colours before applying.");
                return;
            };
            let brightness = selected_brightness.value() as u32;
            let (feature, value) = if selected_kind.selected() == 1 {
                (
                    "per_zone_mode",
                    format!(
                        "{},{},{},{},{}",
                        lighting::hex(parsed[0]),
                        lighting::hex(parsed[1]),
                        lighting::hex(parsed[2]),
                        lighting::hex(parsed[3]),
                        brightness
                    ),
                )
            } else {
                let effect = selected_mode.selected();
                let direction = if matches!(effect, 3 | 4) {
                    selected_direction.selected() + 1
                } else {
                    0
                };
                let speed = if effect >= 2 {
                    selected_speed.value_as_int()
                } else {
                    0
                };
                (
                    "four_zone_mode",
                    format!(
                        "{effect},{speed},{brightness},{direction},{},{},{}",
                        lighting::channel(parsed[0][0]),
                        lighting::channel(parsed[0][1]),
                        lighting::channel(parsed[0][2])
                    ),
                )
            };
            if let Err(error) = predator_core::validate(feature, &value) {
                connection.error(&error);
                return;
            }
            connection.set(feature, value);
        });
        Self {
            widget,
            kind,
            mode,
            speed,
            brightness,
            direction,
            colors,
            apply,
            previous: RefCell::new(None),
        }
    }
    pub fn update(&self, status: &Status) {
        let effect = status
            .saved
            .get("four_zone_mode")
            .or_else(|| status.values.get("four_zone_mode"))
            .cloned();
        let zones = status
            .saved
            .get("per_zone_mode")
            .or_else(|| status.values.get("per_zone_mode"))
            .cloned();
        let per_zone = status.saved.contains_key("per_zone_mode");
        self.apply.set_sensitive(client::supports(
            status,
            if per_zone {
                "per_zone_mode"
            } else {
                "four_zone_mode"
            },
        ));
        let current = LightingSnapshot {
            effect: effect.clone(),
            zones: zones.clone(),
            per_zone,
        };
        if self.previous.borrow().as_ref() == Some(&current) {
            return;
        }
        *self.previous.borrow_mut() = Some(current);
        let has_effect = effect.is_some();
        if let Some(value) = effect {
            let fields: Vec<u32> = value
                .split(',')
                .filter_map(|part| part.parse().ok())
                .collect();
            if fields.len() == 7 {
                self.mode.set_selected(fields[0].min(7));
                self.speed.set_value(fields[1] as f64);
                self.direction
                    .set_selected(if fields[3] == 2 { 1 } else { 0 });
                if !per_zone {
                    self.brightness.set_value(fields[2] as f64);
                    self.colors[0].set_text(&format!(
                        "{:02x}{:02x}{:02x}",
                        fields[4], fields[5], fields[6]
                    ));
                }
            }
        }
        if let Some(value) =
            zones.filter(|_| per_zone || !status.saved.contains_key("four_zone_mode"))
        {
            let fields: Vec<_> = value.split(',').collect();
            if fields.len() == 5 {
                for (index, (entry, value)) in self.colors.iter().zip(&fields).enumerate() {
                    if per_zone || !has_effect || index > 0 {
                        entry.set_text(value);
                    }
                }
                if per_zone {
                    if let Ok(brightness) = fields[4].parse() {
                        self.brightness.set_value(brightness);
                    }
                }
            }
        }
        self.kind.set_selected(u32::from(per_zone));
    }
}
