use crate::{
    client::{self, Client},
    telemetry::Snapshot,
};
use gtk::prelude::*;
use std::{cell::RefCell, collections::VecDeque, rc::Rc};
type TemperatureHistory = Rc<RefCell<VecDeque<(Option<f64>, Option<f64>)>>>;

#[derive(Clone)]
pub struct Dashboard {
    pub widget: gtk::Box,
    system: gtk::Label,
    cpu: gtk::Label,
    gpu: gtk::Label,
    memory: gtk::Label,
    fans: gtk::Label,
    battery: gtk::Label,
    graph: gtk::DrawingArea,
    history: TemperatureHistory,
}
fn value(value: Option<f64>, suffix: &str) -> String {
    value.map_or_else(
        || "Unavailable".into(),
        |value| format!("{value:.0}{suffix}"),
    )
}
impl Dashboard {
    pub fn new(client: &Client) -> Self {
        let widget = client::page("Overview", "Your Predator, at a glance.");
        let grid = gtk::Grid::builder()
            .column_spacing(16)
            .row_spacing(16)
            .column_homogeneous(true)
            .build();
        widget.append(&grid);
        let mut labels = Vec::new();
        for (index, name) in ["System", "CPU", "GPU", "Memory", "Fans", "Battery"]
            .iter()
            .enumerate()
        {
            let label = gtk::Label::new(Some("Reading…"));
            label.set_xalign(0.0);
            label.set_wrap(true);
            label.set_selectable(true);
            label.set_hexpand(true);
            label.set_max_width_chars(36);
            let card = client::card(name, "");
            card.append(&label);
            grid.attach(&card, (index % 2) as i32, (index / 2) as i32, 1, 1);
            labels.push(label);
        }
        let history: TemperatureHistory = Rc::new(RefCell::new(VecDeque::with_capacity(60)));
        let graph = gtk::DrawingArea::builder()
            .content_height(220)
            .hexpand(true)
            .build();
        let samples = history.clone();
        graph.set_draw_func(move |_, context, width, height| {
            let samples = samples.borrow();
            let width = width as f64 - 32.0;
            let height = height as f64 - 32.0;
            context.set_source_rgb(0.15, 0.23, 0.25);
            for temperature in [0.0, 25.0, 50.0, 75.0, 100.0] {
                let y = 16.0 + height * (1.0 - temperature / 110.0);
                context.move_to(16.0, y);
                context.line_to(width + 16.0, y);
                let _ = context.stroke();
            }
            for (gpu, color) in [(false, (0.36, 0.89, 0.75)), (true, (0.70, 0.54, 0.98))] {
                context.set_source_rgb(color.0, color.1, color.2);
                context.set_line_width(2.0);
                let mut connected = false;
                for (index, sample) in samples.iter().enumerate() {
                    let sample = if gpu { sample.1 } else { sample.0 };
                    if let Some(temperature) = sample {
                        let x = 16.0 + index as f64 / 59.0 * width;
                        let y = 16.0 + height * (1.0 - temperature.clamp(0.0, 110.0) / 110.0);
                        if connected {
                            context.line_to(x, y);
                        } else {
                            context.move_to(x, y);
                            connected = true;
                        }
                    } else {
                        connected = false;
                    }
                }
                let _ = context.stroke();
            }
        });
        let chart = client::card("Thermal history", "CPU · teal     GPU · violet");
        chart.append(&graph);
        widget.append(&chart);
        widget.append(&gtk::Label::new(Some(
            "Temperature history · CPU in teal · GPU in violet · 60 samples",
        )));
        let modes = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        for (title, feature, value) in [
            ("Automatic fans", "fan_speed", "0,0"),
            ("Maximum fans", "fan_speed", "100,100"),
            ("Balanced", "profile", "balanced"),
        ] {
            let button = gtk::Button::with_label(title);
            let connection = client.clone();
            button.connect_clicked(move |_| connection.set(feature, value));
            modes.append(&button);
        }
        widget.append(&modes);
        Self {
            widget,
            system: labels[0].clone(),
            cpu: labels[1].clone(),
            gpu: labels[2].clone(),
            memory: labels[3].clone(),
            fans: labels[4].clone(),
            battery: labels[5].clone(),
            graph,
            history,
        }
    }
    pub fn update(&self, snapshot: Snapshot) {
        self.system
            .set_text(&format!("{} · Linux {}", snapshot.os, snapshot.kernel));
        self.cpu.set_text(&format!(
            "{}\n{} · {:.0}% usage",
            snapshot.cpu_name,
            value(snapshot.cpu_temperature, " °C"),
            snapshot.cpu_usage
        ));
        self.gpu.set_text(&format!(
            "{}\n{} · {} usage · driver {}",
            snapshot.gpu_name,
            value(snapshot.gpu_temperature, " °C"),
            value(snapshot.gpu_usage, "%"),
            snapshot.gpu_driver
        ));
        self.memory.set_text(&format!(
            "{:.0}% of {:.1} GiB",
            snapshot.ram_usage, snapshot.ram_total_gib
        ));
        self.fans.set_text(&format!(
            "CPU {} · GPU {}",
            snapshot
                .cpu_fan
                .map_or_else(|| "unavailable".into(), |value| format!("{value} RPM")),
            snapshot
                .gpu_fan
                .map_or_else(|| "unavailable".into(), |value| format!("{value} RPM"))
        ));
        self.battery.set_text(&snapshot.battery.map_or_else(
            || "No battery detected".into(),
            |percentage| {
                format!(
                    "{percentage}% · {} · {}",
                    snapshot.battery_status, snapshot.remaining
                )
            },
        ));
        let mut history = self.history.borrow_mut();
        if history.len() == 60 {
            history.pop_front();
        }
        history.push_back((snapshot.cpu_temperature, snapshot.gpu_temperature));
        drop(history);
        self.graph.queue_draw();
    }
}
