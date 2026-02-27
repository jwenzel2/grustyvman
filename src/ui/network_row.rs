use glib::subclass::prelude::*;
use gtk4 as gtk;
use gtk::prelude::*;
use std::cell::RefCell;

mod imp {
    use super::*;
    use gtk::subclass::prelude::{WidgetImpl, BoxImpl};

    #[derive(Debug, Default)]
    pub struct NetworkRow {
        pub status_dot: RefCell<Option<gtk::Label>>,
        pub name_label: RefCell<Option<gtk::Label>>,
        pub subtitle_label: RefCell<Option<gtk::Label>>,
        pub current_css: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for NetworkRow {
        const NAME: &'static str = "GrustyvmanNetworkRow";
        type Type = super::NetworkRow;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for NetworkRow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.set_orientation(gtk::Orientation::Horizontal);
            obj.set_spacing(12);
            obj.set_margin_top(6);
            obj.set_margin_bottom(6);
            obj.set_margin_start(6);
            obj.set_margin_end(6);

            let dot = gtk::Label::new(Some("\u{25CF}"));
            dot.add_css_class("status-dot");
            obj.append(&dot);
            *self.status_dot.borrow_mut() = Some(dot);

            let text_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
            text_box.set_hexpand(true);

            let name_label = gtk::Label::new(None);
            name_label.set_halign(gtk::Align::Start);
            name_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            name_label.add_css_class("heading");
            text_box.append(&name_label);
            *self.name_label.borrow_mut() = Some(name_label);

            let subtitle_label = gtk::Label::new(None);
            subtitle_label.set_halign(gtk::Align::Start);
            subtitle_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            subtitle_label.add_css_class("dim-label");
            subtitle_label.add_css_class("caption");
            text_box.append(&subtitle_label);
            *self.subtitle_label.borrow_mut() = Some(subtitle_label);

            obj.append(&text_box);
        }
    }

    impl WidgetImpl for NetworkRow {}
    impl BoxImpl for NetworkRow {}
}

glib::wrapper! {
    pub struct NetworkRow(ObjectSubclass<imp::NetworkRow>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl NetworkRow {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn bind(&self, network: &crate::models::network_object::NetworkObject) {
        let imp = self.imp();

        if let Some(ref dot) = *imp.status_dot.borrow() {
            let old_css = imp.current_css.borrow().clone();
            if !old_css.is_empty() {
                dot.remove_css_class(&old_css);
            }
            let new_css = network.state_css();
            dot.add_css_class(&new_css);
            *imp.current_css.borrow_mut() = new_css;
        }

        if let Some(ref label) = *imp.name_label.borrow() {
            label.set_label(&network.name());
        }

        if let Some(ref label) = *imp.subtitle_label.borrow() {
            label.set_label(&network.subtitle());
        }
    }
}
