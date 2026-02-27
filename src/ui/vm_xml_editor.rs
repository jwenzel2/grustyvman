use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub struct VmXmlEditor {
    pub container: gtk::Box,
    text_view: gtk::TextView,
    buffer: gtk::TextBuffer,
    apply_btn: gtk::Button,
    on_apply: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
}

impl VmXmlEditor {
    pub fn new() -> Self {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);

        // Toolbar with Apply button
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        toolbar.set_margin_top(6);
        toolbar.set_margin_bottom(6);
        toolbar.set_margin_start(12);
        toolbar.set_margin_end(12);

        let apply_btn = gtk::Button::with_label("Apply");
        apply_btn.add_css_class("suggested-action");
        apply_btn.set_halign(gtk::Align::End);
        apply_btn.set_hexpand(true);

        toolbar.append(&apply_btn);
        container.append(&toolbar);

        // Text view
        let buffer = gtk::TextBuffer::new(None);
        let text_view = gtk::TextView::with_buffer(&buffer);
        text_view.set_monospace(true);
        text_view.set_editable(true);
        text_view.set_wrap_mode(gtk::WrapMode::None);
        text_view.set_left_margin(12);
        text_view.set_right_margin(12);
        text_view.set_top_margin(6);
        text_view.set_bottom_margin(6);

        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_vexpand(true);
        scrolled.set_hexpand(true);
        scrolled.set_child(Some(&text_view));

        container.append(&scrolled);

        let on_apply: Rc<RefCell<Option<Box<dyn Fn(String)>>>> = Rc::new(RefCell::new(None));

        let on_apply_ref = on_apply.clone();
        let buffer_ref = buffer.clone();
        apply_btn.connect_clicked(move |_| {
            if let Some(ref callback) = *on_apply_ref.borrow() {
                let start = buffer_ref.start_iter();
                let end = buffer_ref.end_iter();
                let text = buffer_ref.text(&start, &end, false).to_string();
                callback(text);
            }
        });

        Self {
            container,
            text_view,
            buffer,
            apply_btn,
            on_apply,
        }
    }

    pub fn set_xml(&self, xml: &str) {
        self.buffer.set_text(xml);
    }

    pub fn set_on_apply(&self, f: impl Fn(String) + 'static) {
        *self.on_apply.borrow_mut() = Some(Box::new(f));
    }
}
