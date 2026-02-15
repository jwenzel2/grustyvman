use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::backend::types::SnapshotInfo;

pub struct VmSnapshotView {
    pub container: gtk::Box,
    snapshots_group: adw::PreferencesGroup,
    #[allow(dead_code)]
    empty_page: adw::StatusPage,
    stack: gtk::Stack,
    on_create: RefCell<Option<Rc<dyn Fn()>>>,
    on_revert: RefCell<Option<Rc<dyn Fn(String)>>>,
    on_delete: RefCell<Option<Rc<dyn Fn(String)>>>,
}

impl VmSnapshotView {
    pub fn new() -> Self {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);

        let stack = gtk::Stack::new();

        // Empty state
        let empty_page = adw::StatusPage::new();
        empty_page.set_title("No Snapshots");
        empty_page.set_description(Some("Create a snapshot to save the current state of this VM"));
        empty_page.set_icon_name(Some("camera-photo-symbolic"));
        stack.add_named(&empty_page, Some("empty"));

        // Snapshots list
        let scrolled = gtk::ScrolledWindow::new();
        let clamp = adw::Clamp::new();
        clamp.set_maximum_size(800);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 24);
        content.set_margin_top(24);
        content.set_margin_bottom(24);
        content.set_margin_start(24);
        content.set_margin_end(24);

        let snapshots_group = adw::PreferencesGroup::new();
        snapshots_group.set_title("Snapshots");
        content.append(&snapshots_group);

        clamp.set_child(Some(&content));
        scrolled.set_child(Some(&clamp));
        stack.add_named(&scrolled, Some("list"));

        stack.set_visible_child_name("empty");
        container.append(&stack);

        Self {
            container,
            snapshots_group,
            empty_page,
            stack,
            on_create: RefCell::new(None),
            on_revert: RefCell::new(None),
            on_delete: RefCell::new(None),
        }
    }

    pub fn set_on_create(&self, f: impl Fn() + 'static) {
        *self.on_create.borrow_mut() = Some(Rc::new(f));
    }

    pub fn set_on_revert(&self, f: impl Fn(String) + 'static) {
        *self.on_revert.borrow_mut() = Some(Rc::new(f));
    }

    pub fn set_on_delete(&self, f: impl Fn(String) + 'static) {
        *self.on_delete.borrow_mut() = Some(Rc::new(f));
    }

    pub fn update(&self, snapshots: &[SnapshotInfo]) {
        // Clear existing rows
        clear_pref_group(&self.snapshots_group);

        // Create button in header suffix
        let create_btn = gtk::Button::from_icon_name("list-add-symbolic");
        create_btn.set_tooltip_text(Some("Create Snapshot"));
        create_btn.set_valign(gtk::Align::Center);
        self.snapshots_group.set_header_suffix(Some(&create_btn));

        if let Some(ref cb) = *self.on_create.borrow() {
            let cb = cb.clone();
            create_btn.connect_clicked(move |_| {
                cb();
            });
        }

        if snapshots.is_empty() {
            self.stack.set_visible_child_name("empty");

            // Also add create button to empty page
            // (the header suffix is on the list view, so we handle empty state separately)
            return;
        }

        self.stack.set_visible_child_name("list");

        for snap in snapshots {
            let row = adw::ActionRow::new();

            // Status dot prefix
            let dot = gtk::Label::new(Some("\u{25CF}"));
            dot.add_css_class("status-dot");
            dot.add_css_class(snap.state.css_class());
            row.add_prefix(&dot);

            // Title: name + (current) badge
            let title = if snap.is_current {
                format!("{} (current)", snap.name)
            } else {
                snap.name.clone()
            };
            row.set_title(&title);

            // Subtitle: date, state, description
            let date_str = format_timestamp(snap.creation_time);
            let mut subtitle = format!("{} \u{2022} {}", date_str, snap.state);
            if !snap.description.is_empty() {
                subtitle.push_str(&format!(" \u{2022} {}", snap.description));
            }
            row.set_subtitle(&subtitle);
            row.set_activatable(false);

            // Revert button
            let revert_btn = gtk::Button::from_icon_name("edit-undo-symbolic");
            revert_btn.set_tooltip_text(Some("Revert to Snapshot"));
            revert_btn.set_valign(gtk::Align::Center);
            revert_btn.add_css_class("flat");

            let snap_name = snap.name.clone();
            if let Some(ref cb) = *self.on_revert.borrow() {
                let cb = cb.clone();
                revert_btn.connect_clicked(move |_| {
                    cb(snap_name.clone());
                });
            }
            row.add_suffix(&revert_btn);

            // Delete button
            let delete_btn = gtk::Button::from_icon_name("edit-delete-symbolic");
            delete_btn.set_tooltip_text(Some("Delete Snapshot"));
            delete_btn.set_valign(gtk::Align::Center);
            delete_btn.add_css_class("flat");

            let snap_name = snap.name.clone();
            if let Some(ref cb) = *self.on_delete.borrow() {
                let cb = cb.clone();
                delete_btn.connect_clicked(move |_| {
                    cb(snap_name.clone());
                });
            }
            row.add_suffix(&delete_btn);

            self.snapshots_group.add(&row);
        }
    }
}

fn format_timestamp(epoch: i64) -> String {
    let datetime = glib::DateTime::from_unix_local(epoch);
    match datetime {
        Ok(dt) => dt
            .format("%Y-%m-%d %H:%M:%S")
            .map(|s| s.to_string())
            .unwrap_or_else(|_| epoch.to_string()),
        Err(_) => epoch.to_string(),
    }
}

fn clear_pref_group(group: &adw::PreferencesGroup) {
    group.set_header_suffix(None::<&gtk::Widget>);

    let mut rows_to_remove = Vec::new();
    let mut child = group.first_child();
    while let Some(c) = child {
        let next = c.next_sibling();
        if c.downcast_ref::<adw::ActionRow>().is_some() {
            rows_to_remove.push(c);
        }
        child = next;
    }
    for row in rows_to_remove {
        group.remove(&row);
    }
}
