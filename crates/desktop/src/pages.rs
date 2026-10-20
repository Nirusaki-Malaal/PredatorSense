//! Create settings panels on first use and retain drafts when switching pages.
use crate::{
    client::Client, dashboard::Dashboard, device::Device, keyboard::Keyboard,
    performance::Performance, telemetry::Snapshot,
};
use gtk::prelude::*;
use predator_core::Status;

pub struct Pages {
    stack: gtk::Stack,
    client: Client,
    dashboard: Dashboard,
    performance: Option<Performance>,
    keyboard: Option<Keyboard>,
    device: Option<Device>,
    latest: Option<Status>,
}

impl Pages {
    pub fn new(client: &Client, stack: &gtk::Stack) -> Self {
        let dashboard = Dashboard::new(client);
        add(stack, "overview", &dashboard.widget);
        Self {
            stack: stack.clone(),
            client: client.clone(),
            dashboard,
            performance: None,
            keyboard: None,
            device: None,
            latest: None,
        }
    }

    pub fn show(&mut self, name: &str) {
        if self.stack.child_by_name(name).is_none() {
            match name {
                "performance" => {
                    let page = Performance::new(&self.client);
                    if let Some(status) = &self.latest {
                        page.update(status);
                    }
                    add(&self.stack, name, &page.widget);
                    self.performance = Some(page);
                }
                "lighting" => {
                    let page = Keyboard::new(&self.client);
                    if let Some(status) = &self.latest {
                        page.update(status);
                    }
                    add(&self.stack, name, &page.widget);
                    self.keyboard = Some(page);
                }
                "device" => {
                    let page = Device::new(&self.client);
                    if let Some(status) = &self.latest {
                        page.update(status);
                    }
                    add(&self.stack, name, &page.widget);
                    self.device = Some(page);
                }
                "diagnostics" => add(&self.stack, name, &crate::diagnostics(&self.client)),
                _ => return,
            }
        }
        self.stack.set_visible_child_name(name);
    }

    pub fn update(&mut self, status: &Status) {
        if let Some(page) = &self.performance {
            page.update(status);
        }
        if let Some(page) = &self.keyboard {
            page.update(status);
        }
        if let Some(page) = &self.device {
            page.update(status);
        }
        self.latest = Some(status.clone());
    }

    pub fn sample(&self, snapshot: Snapshot) {
        self.dashboard.update(snapshot);
    }
}

fn add(stack: &gtk::Stack, name: &str, page: &gtk::Box) {
    page.set_margin_top(28);
    page.set_margin_bottom(28);
    page.set_margin_start(28);
    page.set_margin_end(28);
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(page)
        .build();
    stack.add_named(&scroll, Some(name));
}
