//! Predator Sense: Phantom Studio's sidebar/card visual language for PHN16-71.
#![forbid(unsafe_code)]
mod client;
mod curve_editor;
mod dashboard;
mod device;
mod keyboard;
mod keyboard_preview;
mod launcher;
mod lighting;
mod pages;
mod performance;
mod runtime;
mod telemetry;
use client::Client;
use gtk::{gio, glib, prelude::*};
use predator_core::Request;
use std::{cell::RefCell, rc::Rc, sync::mpsc, time::Duration};
fn label(text: &str, class: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    if !class.is_empty() {
        label.add_css_class(class);
    }
    label
}
fn main() -> glib::ExitCode {
    if std::env::args().any(|argument| argument == "--tray" || argument == "--start-hidden") {
        return match launcher::launch("predator-tray") {
            Ok(()) => glib::ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("Cannot start tray: {error}");
                glib::ExitCode::FAILURE
            }
        };
    }
    if let Err(error) = runtime::prepare() {
        eprintln!("Cannot configure GTK memory retention: {error}");
        return glib::ExitCode::FAILURE;
    }
    let app = gtk::Application::builder()
        .application_id("io.github.predatorsense.Studio")
        .build();
    app.connect_activate(|app| {
        if let Some(window) = app.active_window() {
            window.present();
        } else {
            build(app);
        }
    });
    app.run_with_args(&["predator-sense"])
}
fn build(app: &gtk::Application) {
    let css = gtk::CssProvider::new();
    css.load_from_data(include_str!("style.css"));
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    gtk::style_context_add_provider_for_display(
        &display,
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    if let Some(settings) = gtk::Settings::default() {
        // Standard symbolic assets avoid indexing large user icon collections.
        settings.set_gtk_icon_theme_name(Some("Adwaita"));
        settings.set_gtk_application_prefer_dark_theme(true);
    }
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Predator Sense")
        .default_width(1120)
        .default_height(800)
        .build();
    let header = gtk::HeaderBar::new();
    header.set_title_widget(Some(&label("PREDATOR SENSE", "eyebrow")));
    let sidebar_toggle = gtk::ToggleButton::builder()
        .icon_name("sidebar-show-symbolic")
        .active(true)
        .tooltip_text("Toggle sidebar")
        .build();
    header.pack_start(&sidebar_toggle);
    window.set_titlebar(Some(&header));
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    body.set_vexpand(true);
    let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 8);
    sidebar.add_css_class("sidebar");
    sidebar.set_width_request(190);
    sidebar.append(&label("PREDATOR", "brand"));
    sidebar.append(&label("LAPTOP STUDIO", "eyebrow"));
    let gap = gtk::Box::new(gtk::Orientation::Vertical, 0);
    gap.set_height_request(24);
    sidebar.append(&gap);
    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.set_transition_duration(120);
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    let feedback = label("Connecting to Predator Sense…", "status");
    feedback.set_wrap(true);
    feedback.set_hexpand(true);
    feedback.set_selectable(true);
    let (client, responses) = Client::new(&feedback);
    let views = Rc::new(RefCell::new(pages::Pages::new(&client, &stack)));
    let mut group: Option<gtk::ToggleButton> = None;
    for (name, title, icon) in [
        ("overview", "Overview", "view-grid-symbolic"),
        (
            "performance",
            "Performance",
            "power-profile-performance-symbolic",
        ),
        ("lighting", "Lighting", "input-keyboard-symbolic"),
        ("device", "Device", "preferences-system-symbolic"),
        ("diagnostics", "Diagnostics", "dialog-information-symbolic"),
    ] {
        let button = gtk::ToggleButton::new();
        button.add_css_class("nav");
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        content.append(&gtk::Image::from_icon_name(icon));
        content.append(&label(title, ""));
        button.set_child(Some(&content));
        if let Some(first) = &group {
            button.set_group(Some(first));
        } else {
            button.set_active(true);
            group = Some(button.clone());
        }
        let pages = views.clone();
        button.connect_toggled(move |button| {
            if button.is_active() {
                pages.borrow_mut().show(name);
            }
        });
        sidebar.append(&button);
    }
    views.borrow_mut().show("overview");
    let spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    sidebar.append(&spacer);
    sidebar.append(&label("CONNECTED HARDWARE", "eyebrow"));
    let hardware = label("Predator PHN16-71", "device-name");
    hardware.set_wrap(true);
    sidebar.append(&hardware);
    let badge = label("CONNECTING", "pill");
    badge.set_halign(gtk::Align::Start);
    sidebar.append(&badge);
    let version = label("v0.6.1", "subtle");
    version.set_margin_top(12);
    sidebar.append(&version);
    let weak_sidebar = sidebar.downgrade();
    sidebar_toggle.connect_toggled(move |button| {
        if let Some(sidebar) = weak_sidebar.upgrade() {
            sidebar.set_visible(button.is_active());
        }
    });
    body.append(&sidebar);
    body.append(&stack);
    root.append(&body);
    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    footer.add_css_class("statusbar");
    footer.append(&feedback);
    for (title, request) in [
        ("Refresh", Request::Refresh),
        ("Restore saved", Request::Restore),
    ] {
        let button = gtk::Button::with_label(title);
        let connection = client.clone();
        button.connect_clicked(move |_| connection.send(request.clone()));
        footer.append(&button);
    }
    root.append(&footer);
    window.set_child(Some(&root));
    let (samples, requests) = mpsc::sync_channel(1);
    let (results, metrics) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut sampler = telemetry::Sampler::new();
        while requests.recv().is_ok() {
            if results.send(sampler.sample()).is_err() {
                break;
            }
        }
    });
    let connection = client.clone();
    let pages = stack.downgrade();
    let mut ticks = 0u32;
    let polling = glib::timeout_add_local(Duration::from_millis(100), move || {
        ticks = ticks.wrapping_add(1);
        if ticks.is_multiple_of(10) {
            connection.send(Request::Status);
        }
        if ticks.is_multiple_of(20)
            && pages
                .upgrade()
                .is_some_and(|page| page.visible_child_name().as_deref() == Some("overview"))
        {
            let _ = samples.try_send(());
        }
        while let Ok(completion) = responses.try_recv() {
            match completion.response {
                Ok(response) if response.ok => {
                    if let Some(status) = response.status {
                        hardware.set_text(&status.model);
                        let online = !status.supported.is_empty();
                        badge.set_text(if online {
                            "CONNECTED"
                        } else {
                            "DRIVER OFFLINE"
                        });
                        if online {
                            badge.add_css_class("connected");
                        } else {
                            badge.remove_css_class("connected");
                        }
                        views.borrow_mut().update(&status);
                        if !status.errors.is_empty() {
                            feedback.set_text(&status.errors.join("\n"));
                        } else if !completion.background {
                            feedback.set_text("Settings applied.");
                        } else if feedback.text().starts_with("Connecting")
                            || feedback.text().starts_with("Daemon unavailable")
                        {
                            feedback.set_text("Ready. Settings are saved automatically.");
                        }
                    }
                }
                Ok(response) => {
                    feedback.set_text(response.error.as_deref().unwrap_or("Operation failed"))
                }
                Err(error) => {
                    badge.set_text("DAEMON OFFLINE");
                    badge.remove_css_class("connected");
                    feedback.set_text(&format!(
                        "Daemon unavailable: {error}. Start predator-sense.service."
                    ));
                }
            }
        }
        while let Ok(snapshot) = metrics.try_recv() {
            views.borrow().sample(snapshot);
        }
        glib::ControlFlow::Continue
    });
    let sources = Rc::new(RefCell::new(Some(polling)));
    let cleanup = sources.clone();
    window.connect_close_request(move |_| {
        if let Some(source) = cleanup.borrow_mut().take() {
            source.remove();
        }
        glib::Propagation::Proceed
    });
    let quit = gio::SimpleAction::new("quit", None);
    let weak = app.downgrade();
    quit.connect_activate(move |_, _| {
        if let Some(app) = weak.upgrade() {
            app.quit();
        }
    });
    app.add_action(&quit);
    app.set_accels_for_action("app.quit", &["<Primary>q"]);
    client.send(Request::Status);
    window.present();
    if std::env::var_os("PREDATOR_SENSE_DIAGNOSTICS").is_some() {
        if let Some(renderer) = window.renderer() {
            eprintln!("Predator Sense renderer: {}", renderer.type_().name());
        }
    }
}
fn diagnostics(client: &Client) -> gtk::Box {
    let page = client::page("Diagnostics", "A focused build for your Predator PHN16-71.");
    let card = client::card(
        "Hardware event path",
        "The physical mode button reaches the driver through the Embedded Controller.",
    );
    let state = std::fs::read_to_string("/sys/firmware/acpi/interrupts/gpe6E")
        .unwrap_or_else(|_| "EC interrupt state unavailable".into());
    let text = label(&format!("EC interrupt 0x6E: {}", state.trim()), "muted");
    text.set_wrap(true);
    card.append(&text);
    let note = label("Disabling the EC interrupt blocks mode-button events. Avoid wakeup workarounds that disable GPE 0x6E on this laptop.", "muted");
    note.set_wrap(true);
    card.append(&note);
    page.append(&card);
    let restore = gtk::Button::with_label("Restore saved preferences");
    let client = client.clone();
    restore.connect_clicked(move |_| client.send(Request::Restore));
    page.append(&restore);
    page
}
