use crate::{
    client::{self, Client},
    curve_editor::CurveEditor,
};
use gtk::prelude::*;
use predator_core::Status;
use std::{cell::RefCell, rc::Rc};
#[derive(Clone)]
pub struct Performance {
    pub widget: gtk::Box,
    profile: gtk::DropDown,
    choices: Rc<RefCell<Vec<String>>>,
    apply: gtk::Button,
    cpu: gtk::SpinButton,
    gpu: gtk::SpinButton,
    fans: gtk::Box,
    curve_profile: gtk::DropDown,
    curve: CurveEditor,
    curve_controls: gtk::Box,
    previous: RefCell<std::collections::BTreeMap<String, String>>,
}
impl Performance {
    pub fn new(client: &Client) -> Self {
        let widget = client::page(
            "Power & performance",
            "Choose a power profile and tune CPU and GPU cooling.",
        );
        let profile = gtk::DropDown::from_strings(&[]);
        profile.set_hexpand(true);
        let profile_card = client::card(
            "Power profile",
            "Profiles are remembered separately for AC and battery power.",
        );
        widget.append(&profile_card);
        client::labeled(&profile_card, "Thermal profile", &profile);
        let choices = Rc::new(RefCell::new(Vec::<String>::new()));
        let apply = gtk::Button::with_label("Apply profile");
        apply.add_css_class("suggested-action");
        profile_card.append(&apply);
        let selected = profile.clone();
        let profiles = choices.clone();
        let connection = client.clone();
        apply.connect_clicked(move |_| {
            if let Some(profile) = profiles.borrow().get(selected.selected() as usize) {
                connection.set("profile", profile);
            }
        });
        let fans = client::card("Fan control", "Balance acoustic comfort and cooling.");
        let shortcuts = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        for (label, value) in [("Automatic", "0,0"), ("Maximum", "100,100")] {
            let button = gtk::Button::with_label(label);
            let connection = client.clone();
            button.connect_clicked(move |_| connection.set("fan_speed", value));
            shortcuts.append(&button);
        }
        fans.append(&shortcuts);
        let cpu = client::spin(0.0, 100.0, 0.0);
        let gpu = client::spin(0.0, 100.0, 0.0);
        client::labeled(&fans, "CPU fan (%)", &cpu);
        client::labeled(&fans, "GPU fan (%)", &gpu);
        let manual = gtk::Button::with_label("Apply manual fans");
        fans.append(&manual);
        let connection = client.clone();
        let cpu_control = cpu.clone();
        let gpu_control = gpu.clone();
        manual.connect_clicked(move |_| {
            connection.set(
                "fan_speed",
                format!(
                    "{},{}",
                    cpu_control.value_as_int(),
                    gpu_control.value_as_int()
                ),
            )
        });
        let note = gtk::Label::new(Some(
            "0 keeps that fan under automatic control. CPU and GPU can be configured separately.",
        ));
        note.set_wrap(true);
        note.set_xalign(0.0);
        fans.append(&note);
        widget.append(&fans);
        let curve_controls = client::card(
            "Temperature curve",
            "Control how cooling responds to rising temperatures.",
        );
        let curve_profile = gtk::DropDown::from_strings(&["Conservative", "Aggressive", "Custom"]);
        client::labeled(&curve_controls, "Fan curve", &curve_profile);
        let curve = CurveEditor::new();
        curve_controls.append(&curve.widget);
        let enable = gtk::Button::with_label("Apply and enable curve");
        curve_controls.append(&enable);
        let connection = client.clone();
        let selector = curve_profile.clone();
        let points = curve.text.clone();
        enable.connect_clicked(move |_| {
            let profile = match selector.selected() {
                0 => "conservative",
                1 => "aggressive",
                _ => "custom",
            };
            if profile == "custom" {
                if let Err(error) = predator_core::validate("fan_curve_points", &points.text()) {
                    connection.error(&error);
                    return;
                }
            }
            connection.send(predator_core::Request::ConfigureCurve {
                profile: profile.into(),
                points: points.text().into(),
            });
        });
        widget.append(&curve_controls);
        Self {
            widget,
            profile,
            choices,
            apply,
            cpu,
            gpu,
            fans,
            curve_profile,
            curve,
            curve_controls,
            previous: RefCell::new(std::collections::BTreeMap::new()),
        }
    }
    pub fn update(&self, status: &Status) {
        let display: Vec<_> = status
            .choices
            .iter()
            .map(|profile| match profile.as_str() {
                "low-power" => "Eco",
                "quiet" => "Quiet",
                "balanced" => "Balanced",
                "balanced-performance" => "Performance",
                "performance" => "Turbo",
                other => other,
            })
            .collect();
        let choices_changed = *self.choices.borrow() != status.choices;
        if choices_changed {
            self.profile
                .set_model(Some(&gtk::StringList::new(&display)));
            *self.choices.borrow_mut() = status.choices.clone();
        }
        let previous = self.previous.borrow();
        let changed = |key: &str| previous.get(key) != status.values.get(key);
        if changed("profile") || choices_changed {
            if let Some(index) = status
                .choices
                .iter()
                .position(|profile| Some(profile) == status.values.get("profile"))
            {
                self.profile.set_selected(index as u32);
            }
        }
        self.apply
            .set_sensitive(client::supports(status, "profile"));
        self.fans
            .set_sensitive(client::supports(status, "fan_speed"));
        self.curve_controls.set_sensitive(
            client::supports(status, "thermal_fan_mode")
                && client::supports(status, "fan_curve_points"),
        );
        if let Some(value) = status
            .values
            .get("fan_speed")
            .filter(|_| changed("fan_speed"))
        {
            let values: Vec<_> = value
                .split(',')
                .filter_map(|value| value.parse::<f64>().ok())
                .collect();
            if values.len() == 2 {
                self.cpu.set_value(values[0]);
                self.gpu.set_value(values[1]);
            }
        }
        if let Some(profile) = status
            .values
            .get("fan_curve_profile")
            .filter(|_| changed("fan_curve_profile"))
        {
            self.curve_profile.set_selected(match profile.as_str() {
                "aggressive" => 1,
                "custom" => 2,
                _ => 0,
            });
        }
        if let Some(points) = status
            .values
            .get("fan_curve_points")
            .filter(|_| changed("fan_curve_points"))
        {
            self.curve.update(points);
        }
        drop(previous);
        *self.previous.borrow_mut() = status.values.clone();
    }
}
