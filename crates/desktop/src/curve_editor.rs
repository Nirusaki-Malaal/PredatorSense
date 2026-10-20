//! Bounded custom-curve editor with independent CPU/GPU series and dragging.
use gtk::prelude::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
#[derive(Clone)]
pub struct CurveEditor {
    pub widget: gtk::Box,
    pub text: gtk::Entry,
    graph: gtk::DrawingArea,
}
fn points(text: &str) -> Option<Vec<[f64; 3]>> {
    if predator_core::validate("fan_curve_points", text).is_err() {
        return None;
    }
    text.split(',')
        .map(|point| {
            let fields: Vec<_> = point
                .split(':')
                .map(str::parse::<f64>)
                .collect::<Result<_, _>>()
                .ok()?;
            Some([fields[0], fields[1], fields[2]])
        })
        .collect()
}
fn serialize(points: &[[f64; 3]]) -> String {
    points
        .iter()
        .map(|point| {
            format!(
                "{}:{}:{}",
                point[0] as i32, point[1] as i32, point[2] as i32
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}
impl CurveEditor {
    pub fn new() -> Self {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 10);
        let text = gtk::Entry::builder()
            .text("55:0:0,65:35:35,75:55:60,85:80:85,90:100:100")
            .max_length(256)
            .hexpand(true)
            .build();
        let graph = gtk::DrawingArea::builder()
            .content_width(600)
            .content_height(220)
            .hexpand(true)
            .build();
        widget.append(&graph);
        widget.append(&text);
        let hint=gtk::Label::new(Some("Drag a CPU or GPU point, or edit temperature:CPU:GPU pairs below. Teal = CPU · violet = GPU."));
        hint.set_wrap(true);
        hint.set_xalign(0.0);
        widget.append(&hint);
        let draw_text = text.clone();
        graph.set_draw_func(move |_, context, width, height| {
            let left = 36.0;
            let top = 14.0;
            let width = width as f64 - 52.0;
            let height = height as f64 - 42.0;
            context.set_source_rgb(0.13, 0.19, 0.21);
            for value in [0.0, 25.0, 50.0, 75.0, 100.0] {
                let y = top + height * (1.0 - value / 100.0);
                context.move_to(left, y);
                context.line_to(left + width, y);
                let _ = context.stroke();
                context.move_to(3.0, y + 4.0);
                let _ = context.show_text(&format!("{value:.0}"));
            }
            let Some(points) = points(&draw_text.text()) else {
                return;
            };
            for (series, color) in [(1, (0.36, 0.89, 0.75)), (2, (0.70, 0.54, 0.98))] {
                context.set_source_rgb(color.0, color.1, color.2);
                context.set_line_width(2.0);
                for (index, point) in points.iter().enumerate() {
                    let x = left + (point[0] - 30.0) / 70.0 * width;
                    let y = top + (1.0 - point[series] / 100.0) * height;
                    if index == 0 {
                        context.move_to(x, y);
                    } else {
                        context.line_to(x, y);
                    }
                }
                let _ = context.stroke();
                for point in &points {
                    context.arc(
                        left + (point[0] - 30.0) / 70.0 * width,
                        top + (1.0 - point[series] / 100.0) * height,
                        4.0,
                        0.0,
                        std::f64::consts::TAU,
                    );
                    let _ = context.fill();
                }
            }
        });
        let redraw = graph.downgrade();
        text.connect_changed(move |_| {
            if let Some(graph) = redraw.upgrade() {
                graph.queue_draw();
            }
        });
        let drag = gtk::GestureDrag::new();
        let selected = Rc::new(Cell::new(None::<(usize, usize)>));
        let origin = Rc::new(Cell::new((0.0, 0.0)));
        let snapshot = Rc::new(RefCell::new(Vec::<[f64; 3]>::new()));
        let begin_selected = selected.clone();
        let begin_origin = origin.clone();
        let begin_snapshot = snapshot.clone();
        let begin_text = text.clone();
        let begin_graph = graph.downgrade();
        drag.connect_drag_begin(move |_, x, y| {
            begin_origin.set((x, y));
            let Some(values) = points(&begin_text.text()) else {
                return;
            };
            let Some(graph) = begin_graph.upgrade() else {
                return;
            };
            let width = graph.width() as f64 - 52.0;
            let height = graph.height() as f64 - 42.0;
            let mut nearest = None;
            let mut distance = 20.0f64;
            for (index, point) in values.iter().enumerate() {
                for series in [1, 2] {
                    let px = 36.0 + (point[0] - 30.0) / 70.0 * width;
                    let py = 14.0 + (1.0 - point[series] / 100.0) * height;
                    let current = (x - px).hypot(y - py);
                    if current < distance {
                        distance = current;
                        nearest = Some((index, series));
                    }
                }
            }
            begin_selected.set(nearest);
            *begin_snapshot.borrow_mut() = values;
        });
        let drag_text = text.clone();
        let drag_graph = graph.downgrade();
        drag.connect_drag_update(move |_, dx, dy| {
            let Some((index, series)) = selected.get() else {
                return;
            };
            let Some(graph) = drag_graph.upgrade() else {
                return;
            };
            let mut values = snapshot.borrow().clone();
            let (x, y) = origin.get();
            let width = graph.width() as f64 - 52.0;
            let height = graph.height() as f64 - 42.0;
            if width <= 0.0 || height <= 0.0 {
                return;
            }
            let lower = if index == 0 {
                30.0
            } else {
                values[index - 1][0] + 1.0
            };
            let upper = if index + 1 == values.len() {
                100.0
            } else {
                values[index + 1][0] - 1.0
            };
            values[index][0] = (30.0 + (x + dx - 36.0) / width * 70.0)
                .round()
                .clamp(lower, upper);
            values[index][series] = ((1.0 - (y + dy - 14.0) / height) * 100.0)
                .round()
                .clamp(0.0, 100.0);
            drag_text.set_text(&serialize(&values));
        });
        graph.add_controller(drag);
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let add = gtk::Button::with_label("Add point");
        let remove = gtk::Button::with_label("Remove last point");
        buttons.append(&add);
        buttons.append(&remove);
        widget.append(&buttons);
        let add_text = text.clone();
        add.connect_clicked(move |_| {
            let Some(mut values) = points(&add_text.text()) else {
                return;
            };
            if values.len() >= 8 {
                return;
            }
            let last = values.last().unwrap();
            if last[0] >= 100.0 {
                return;
            }
            values.push([(last[0] + 5.0).min(100.0), last[1], last[2]]);
            add_text.set_text(&serialize(&values));
        });
        let remove_text = text.clone();
        remove.connect_clicked(move |_| {
            if let Some(mut values) = points(&remove_text.text()) {
                if values.len() > 2 {
                    values.pop();
                    remove_text.set_text(&serialize(&values));
                }
            }
        });
        Self {
            widget,
            text,
            graph,
        }
    }
    pub fn update(&self, text: &str) {
        if points(text).is_some() {
            self.text.set_text(text);
            self.graph.queue_draw();
        }
    }
}
