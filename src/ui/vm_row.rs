use glib::subclass::prelude::*;
use gtk4 as gtk;
use gtk::prelude::*;
use std::cell::RefCell;

mod imp {
    use super::*;
    use gtk::subclass::prelude::{WidgetImpl, BoxImpl};

    #[derive(Debug, Default)]
    pub struct VmRow {
        pub status_dot: RefCell<Option<gtk::Label>>,
        pub name_label: RefCell<Option<gtk::Label>>,
        pub subtitle_label: RefCell<Option<gtk::Label>>,
        pub current_css: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for VmRow {
        const NAME: &'static str = "GrustyvmanVmRow";
        type Type = super::VmRow;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for VmRow {
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

    impl WidgetImpl for VmRow {}
    impl BoxImpl for VmRow {}
}

glib::wrapper! {
    pub struct VmRow(ObjectSubclass<imp::VmRow>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl VmRow {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn bind(&self, vm: &crate::models::vm_object::VmObject) {
        // ── Initial values ───────────────────────────────────────────────
        self.apply_state_css(vm.state_css());
        if let Some(ref label) = *self.imp().name_label.borrow() {
            label.set_label(&vm.name());
        }
        if let Some(ref label) = *self.imp().subtitle_label.borrow() {
            label.set_label(&vm.subtitle());
        }

        // ── Live update: state dot color ─────────────────────────────────
        let row = self.clone();
        vm.connect_state_css_notify(move |vm| {
            row.apply_state_css(vm.state_css());
        });

        // ── Live update: subtitle (vCPUs / memory / state text) ──────────
        let row = self.clone();
        vm.connect_subtitle_notify(move |vm| {
            if let Some(ref label) = *row.imp().subtitle_label.borrow() {
                label.set_label(&vm.subtitle());
            }
        });

        // ── Live update: name (rename support) ───────────────────────────
        let row = self.clone();
        vm.connect_name_notify(move |vm| {
            if let Some(ref label) = *row.imp().name_label.borrow() {
                label.set_label(&vm.name());
            }
        });
    }

    fn apply_state_css(&self, new_css: String) {
        let imp = self.imp();
        if let Some(ref dot) = *imp.status_dot.borrow() {
            let old_css = imp.current_css.borrow().clone();
            if !old_css.is_empty() {
                dot.remove_css_class(&old_css);
            }
            dot.add_css_class(&new_css);
            *imp.current_css.borrow_mut() = new_css;
        }
    }
}
