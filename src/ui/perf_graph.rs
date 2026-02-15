use gtk4 as gtk;
use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

pub struct PerfGraph {
    pub widget: gtk::DrawingArea,
    data: Rc<RefCell<VecDeque<f64>>>,
    max_points: usize,
}

impl PerfGraph {
    pub fn new(
        label: &str,
        unit: &str,
        color: (f64, f64, f64),
        max_points: usize,
        fixed_max: Option<f64>,
    ) -> Self {
        let data: Rc<RefCell<VecDeque<f64>>> = Rc::new(RefCell::new(VecDeque::with_capacity(max_points)));
        let widget = gtk::DrawingArea::new();
        widget.set_content_height(120);
        widget.set_hexpand(true);

        let draw_data = data.clone();
        let draw_label = label.to_string();
        let draw_unit = unit.to_string();
        let draw_color = color;
        let draw_fixed_max = fixed_max;

        widget.set_draw_func(move |_area, cr, width, height| {
            let data = draw_data.borrow();
            let w = width as f64;
            let h = height as f64;

            // Background
            cr.set_source_rgba(0.0, 0.0, 0.0, 0.05);
            cr.rectangle(0.0, 0.0, w, h);
            let _ = cr.fill();

            // Determine max value for scaling
            let max_val = if let Some(fm) = draw_fixed_max {
                fm
            } else {
                let data_max = data.iter().cloned().fold(0.0_f64, f64::max);
                if data_max < 1.0 { 1.0 } else { data_max * 1.2 }
            };

            // Grid lines at 25%, 50%, 75%
            cr.set_source_rgba(0.5, 0.5, 0.5, 0.15);
            cr.set_line_width(1.0);
            for frac in &[0.25, 0.50, 0.75] {
                let y = h * (1.0 - frac);
                cr.move_to(0.0, y);
                cr.line_to(w, y);
                let _ = cr.stroke();
            }

            if data.len() >= 2 {
                let n = data.len();
                let step = w / (n as f64 - 1.0);

                // Filled area
                cr.move_to(0.0, h);
                for (i, val) in data.iter().enumerate() {
                    let x = i as f64 * step;
                    let y = h - (val / max_val * h).min(h);
                    cr.line_to(x, y);
                }
                cr.line_to((n as f64 - 1.0) * step, h);
                cr.close_path();
                cr.set_source_rgba(draw_color.0, draw_color.1, draw_color.2, 0.15);
                let _ = cr.fill();

                // Line on top
                cr.set_line_width(2.0);
                cr.set_source_rgba(draw_color.0, draw_color.1, draw_color.2, 0.9);
                for (i, val) in data.iter().enumerate() {
                    let x = i as f64 * step;
                    let y = h - (val / max_val * h).min(h);
                    if i == 0 {
                        cr.move_to(x, y);
                    } else {
                        cr.line_to(x, y);
                    }
                }
                let _ = cr.stroke();
            }

            // Current value text
            if let Some(current) = data.back() {
                let text = if draw_fixed_max.is_some() {
                    format!("{:.1}{}", current, draw_unit)
                } else {
                    format_rate(*current, &draw_unit)
                };
                cr.set_source_rgba(0.8, 0.8, 0.8, 0.9);
                cr.set_font_size(11.0);
                let extents = cr.text_extents(&text).unwrap();
                cr.move_to(w - extents.width() - 8.0, 16.0);
                let _ = cr.show_text(&text);
            }

            // Label in top-left
            cr.set_source_rgba(0.6, 0.6, 0.6, 0.8);
            cr.set_font_size(11.0);
            cr.move_to(8.0, 16.0);
            let _ = cr.show_text(&draw_label);
        });

        Self {
            widget,
            data,
            max_points,
        }
    }

    pub fn push_value(&self, value: f64) {
        let mut data = self.data.borrow_mut();
        if data.len() >= self.max_points {
            data.pop_front();
        }
        data.push_back(value);
        drop(data);
        self.widget.queue_draw();
    }

    pub fn clear(&self) {
        self.data.borrow_mut().clear();
        self.widget.queue_draw();
    }
}

fn format_rate(bytes_sec: f64, _unit: &str) -> String {
    if bytes_sec >= 1_073_741_824.0 {
        format!("{:.1} GB/s", bytes_sec / 1_073_741_824.0)
    } else if bytes_sec >= 1_048_576.0 {
        format!("{:.1} MB/s", bytes_sec / 1_048_576.0)
    } else if bytes_sec >= 1024.0 {
        format!("{:.1} KB/s", bytes_sec / 1024.0)
    } else {
        format!("{:.0} B/s", bytes_sec)
    }
}
