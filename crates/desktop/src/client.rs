//! One bounded IPC worker; status requests are coalesced and never block GTK.
#![forbid(unsafe_code)]
use gtk::prelude::*;
use predator_core::{Request, Response};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender},
    Arc,
};
pub struct Completion {
    pub response: Result<Response, String>,
    pub background: bool,
}
#[derive(Clone)]
pub struct Client {
    sender: SyncSender<Request>,
    feedback: gtk::Label,
    status_pending: Arc<AtomicBool>,
}
impl Client {
    pub fn new(feedback: &gtk::Label) -> (Self, Receiver<Completion>) {
        let (sender, requests) = mpsc::sync_channel::<Request>(8);
        let (results, receiver) = mpsc::sync_channel(8);
        let pending = Arc::new(AtomicBool::new(false));
        let status_pending = pending.clone();
        std::thread::spawn(move || {
            while let Ok(request) = requests.recv() {
                let background = matches!(request, Request::Status);
                let response = predator_core::call(&predator_core::socket_path(), &request)
                    .map_err(|error| error.to_string());
                if background {
                    pending.store(false, Ordering::Release);
                }
                if results
                    .send(Completion {
                        response,
                        background,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        (
            Self {
                sender,
                feedback: feedback.clone(),
                status_pending,
            },
            receiver,
        )
    }
    pub fn send(&self, request: Request) {
        let background = matches!(request, Request::Status);
        if background && self.status_pending.swap(true, Ordering::AcqRel) {
            return;
        }
        match self.sender.try_send(request) {
            Ok(()) if !background => self.feedback.set_text("Working…"),
            Ok(()) => {}
            Err(_) => {
                if background {
                    self.status_pending.store(false, Ordering::Release);
                } else {
                    self.feedback
                        .set_text("An operation is pending. Please wait.");
                }
            }
        }
    }
    pub fn set(&self, feature: &str, value: impl ToString) {
        self.send(Request::Set {
            feature: feature.into(),
            value: value.to_string(),
        });
    }
    pub fn error(&self, error: &str) {
        self.feedback.set_text(error);
    }
}
pub fn supports(status: &predator_core::Status, feature: &str) -> bool {
    status.supported.iter().any(|value| value == feature)
}
pub fn page(title: &str, description: &str) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 20);
    let heading = gtk::Label::new(Some(title));
    heading.add_css_class("page-title");
    heading.set_xalign(0.0);
    page.append(&heading);
    let subtitle = gtk::Label::new(Some(description));
    subtitle.set_wrap(true);
    subtitle.set_xalign(0.0);
    subtitle.add_css_class("muted");
    page.append(&subtitle);
    page
}
pub fn card(title: &str, description: &str) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 12);
    card.add_css_class("card");
    let heading = gtk::Label::new(Some(title));
    heading.add_css_class("section-title");
    heading.set_xalign(0.0);
    card.append(&heading);
    if !description.is_empty() {
        let note = gtk::Label::new(Some(description));
        note.set_wrap(true);
        note.add_css_class("muted");
        note.set_xalign(0.0);
        card.append(&note);
    }
    card
}
pub fn labeled(parent: &gtk::Box, title: &str, widget: &impl IsA<gtk::Widget>) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    row.set_valign(gtk::Align::Center);
    let label = gtk::Label::new(Some(title));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    label.set_wrap(true);
    widget.set_valign(gtk::Align::Center);
    row.append(&label);
    row.append(widget);
    parent.append(&row);
}
pub fn spin(minimum: f64, maximum: f64, value: f64) -> gtk::SpinButton {
    let spin = gtk::SpinButton::with_range(minimum, maximum, 1.0);
    spin.set_value(value);
    spin
}
