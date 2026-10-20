use crate::client::{self, Client};
use gtk::{glib, prelude::*};
use predator_core::Status;
use std::{cell::Cell, rc::Rc};
#[derive(Clone)]
pub struct Device {
    pub widget: gtk::Box,
    syncing: Rc<Cell<bool>>,
    switches: Vec<(&'static str, gtk::Switch)>,
    usb: gtk::DropDown,
    usb_apply: gtk::Button,
    calibration: gtk::Box,
    previous_usb: std::cell::RefCell<Option<String>>,
}
impl Device {
    pub fn new(client: &Client) -> Self {
        let widget = client::page(
            "Device",
            "Battery, display, USB charging and keyboard timeout controls.",
        );
        let settings = client::card("Everyday settings", "Small adjustments for your laptop.");
        widget.append(&settings);
        let syncing = Rc::new(Cell::new(false));
        let mut switches = Vec::new();
        for (feature, title) in [
            ("battery_limiter", "Limit battery charging to 80%"),
            ("backlight_timeout", "Keyboard inactivity timeout"),
            ("boot_animation_sound", "Boot animation sound"),
            ("lcd_override", "LCD overdrive"),
        ] {
            let switch = gtk::Switch::new();
            client::labeled(&settings, title, &switch);
            let connection = client.clone();
            let updating = syncing.clone();
            switch.connect_state_set(move |_, enabled| {
                if updating.get() {
                    return glib::Propagation::Proceed;
                }
                connection.set(feature, if enabled { "1" } else { "0" });
                glib::Propagation::Stop
            });
            switches.push((feature, switch));
        }
        let usb = gtk::DropDown::from_strings(&[
            "Disabled",
            "10% battery threshold",
            "20% battery threshold",
            "30% battery threshold",
        ]);
        client::labeled(&settings, "USB charging while off", &usb);
        let usb_apply = gtk::Button::with_label("Apply USB charging");
        settings.append(&usb_apply);
        let selector = usb.clone();
        let connection = client.clone();
        usb_apply
            .connect_clicked(move |_| connection.set("usb_charging", selector.selected() * 10));
        let calibration = client::card(
            "Battery calibration",
            "Run only when the battery charge estimate needs correcting.",
        );
        let note = gtk::Label::new(Some(
            "Calibration fully charges and discharges the battery. Keep AC power connected.",
        ));
        note.set_xalign(0.0);
        note.set_wrap(true);
        calibration.append(&note);
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        for (title, value) in [("Start calibration", "1"), ("Stop calibration", "0")] {
            let button = gtk::Button::with_label(title);
            let connection = client.clone();
            button.connect_clicked(move |_| connection.set("battery_calibration", value));
            buttons.append(&button);
        }
        calibration.append(&buttons);
        widget.append(&calibration);
        Self {
            widget,
            syncing,
            switches,
            usb,
            usb_apply,
            calibration,
            previous_usb: std::cell::RefCell::new(None),
        }
    }
    pub fn update(&self, status: &Status) {
        self.syncing.set(true);
        for (feature, switch) in &self.switches {
            switch.set_sensitive(client::supports(status, feature));
            let enabled = status
                .values
                .get(*feature)
                .is_some_and(|value| value == "1");
            switch.set_state(enabled);
            switch.set_active(enabled);
        }
        self.syncing.set(false);
        self.usb_apply
            .set_sensitive(client::supports(status, "usb_charging"));
        self.usb
            .set_sensitive(client::supports(status, "usb_charging"));
        if *self.previous_usb.borrow() != status.values.get("usb_charging").cloned() {
            if let Some(value) = status
                .values
                .get("usb_charging")
                .and_then(|value| value.parse::<u32>().ok())
            {
                self.usb.set_selected((value / 10).min(3));
            }
            *self.previous_usb.borrow_mut() = status.values.get("usb_charging").cloned();
        }
        self.calibration
            .set_sensitive(client::supports(status, "battery_calibration"));
    }
}
