use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;

use crate::backend::types::{format_bytes, PoolInfo, VolumeInfo};

pub struct PoolDetailsView {
    pub container: gtk::Box,
    state_row: adw::ActionRow,
    uuid_row: adw::ActionRow,
    type_row: adw::ActionRow,
    path_row: adw::ActionRow,
    autostart_switch: gtk::Switch,
    #[allow(dead_code)]
    autostart_row: adw::ActionRow,
    persistent_row: adw::ActionRow,
    capacity_row: adw::ActionRow,
    allocation_row: adw::ActionRow,
    available_row: adw::ActionRow,
    level_bar: gtk::LevelBar,
    volumes_group: adw::PreferencesGroup,
    /// Rows currently displayed in volumes_group — tracked so we can remove
    /// them reliably on the next update() call without walking the widget tree.
    volume_rows: std::cell::RefCell<Vec<adw::ActionRow>>,
    pub btn_start: gtk::Button,
    pub btn_stop: gtk::Button,
    pub btn_refresh: gtk::Button,
    pub btn_delete: gtk::Button,
    on_add_volume: std::cell::RefCell<Option<std::rc::Rc<dyn Fn()>>>,
    on_upload_volume: std::cell::RefCell<Option<std::rc::Rc<dyn Fn()>>>,
    on_delete_volume: std::cell::RefCell<Option<std::rc::Rc<dyn Fn(String)>>>,
    on_set_autostart: std::cell::RefCell<Option<std::rc::Rc<dyn Fn(bool)>>>,
}

impl PoolDetailsView {
    pub fn new() -> Self {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 24);
        container.set_margin_top(24);
        container.set_margin_bottom(24);
        container.set_margin_start(24);
        container.set_margin_end(24);

        // Action buttons
        let actions_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        actions_box.set_halign(gtk::Align::Center);

        let btn_start = gtk::Button::new();
        btn_start.set_icon_name("media-playback-start-symbolic");
        btn_start.set_tooltip_text(Some("Start Pool"));

        let btn_stop = gtk::Button::new();
        btn_stop.set_icon_name("media-playback-stop-symbolic");
        btn_stop.set_tooltip_text(Some("Stop Pool"));

        let btn_refresh = gtk::Button::new();
        btn_refresh.set_icon_name("view-refresh-symbolic");
        btn_refresh.set_tooltip_text(Some("Refresh Pool"));

        let btn_delete = gtk::Button::new();
        btn_delete.set_icon_name("user-trash-symbolic");
        btn_delete.set_tooltip_text(Some("Delete Pool"));
        btn_delete.add_css_class("destructive-action");

        actions_box.append(&btn_start);
        actions_box.append(&btn_stop);
        actions_box.append(&btn_refresh);
        actions_box.append(&btn_delete);
        container.append(&actions_box);

        // Status group
        let status_group = adw::PreferencesGroup::new();
        status_group.set_title("Status");

        let state_row = adw::ActionRow::new();
        state_row.set_title("State");
        state_row.set_activatable(false);
        status_group.add(&state_row);

        let uuid_row = adw::ActionRow::new();
        uuid_row.set_title("UUID");
        uuid_row.set_activatable(false);
        status_group.add(&uuid_row);

        let type_row = adw::ActionRow::new();
        type_row.set_title("Type");
        type_row.set_activatable(false);
        status_group.add(&type_row);

        let path_row = adw::ActionRow::new();
        path_row.set_title("Path");
        path_row.set_activatable(false);
        status_group.add(&path_row);

        let autostart_row = adw::ActionRow::new();
        autostart_row.set_title("Autostart");
        autostart_row.set_activatable(false);
        let autostart_switch = gtk::Switch::new();
        autostart_switch.set_valign(gtk::Align::Center);
        autostart_row.add_suffix(&autostart_switch);
        status_group.add(&autostart_row);

        let persistent_row = adw::ActionRow::new();
        persistent_row.set_title("Persistent");
        persistent_row.set_activatable(false);
        status_group.add(&persistent_row);

        container.append(&status_group);

        // Capacity group
        let capacity_group = adw::PreferencesGroup::new();
        capacity_group.set_title("Capacity");

        let level_bar = gtk::LevelBar::new();
        level_bar.set_min_value(0.0);
        level_bar.set_max_value(1.0);
        level_bar.set_margin_start(12);
        level_bar.set_margin_end(12);
        level_bar.set_margin_top(6);
        level_bar.set_margin_bottom(6);
        capacity_group.add(&level_bar);

        let capacity_row = adw::ActionRow::new();
        capacity_row.set_title("Capacity");
        capacity_row.set_activatable(false);
        capacity_group.add(&capacity_row);

        let allocation_row = adw::ActionRow::new();
        allocation_row.set_title("Allocation");
        allocation_row.set_activatable(false);
        capacity_group.add(&allocation_row);

        let available_row = adw::ActionRow::new();
        available_row.set_title("Available");
        available_row.set_activatable(false);
        capacity_group.add(&available_row);

        container.append(&capacity_group);

        // Volumes group
        let volumes_group = adw::PreferencesGroup::new();
        volumes_group.set_title("Volumes");
        container.append(&volumes_group);

        Self {
            container,
            state_row,
            uuid_row,
            type_row,
            path_row,
            autostart_switch,
            autostart_row,
            persistent_row,
            capacity_row,
            allocation_row,
            available_row,
            level_bar,
            volumes_group,
            volume_rows: std::cell::RefCell::new(Vec::new()),
            btn_start,
            btn_stop,
            btn_refresh,
            btn_delete,
            on_add_volume: std::cell::RefCell::new(None),
            on_upload_volume: std::cell::RefCell::new(None),
            on_delete_volume: std::cell::RefCell::new(None),
            on_set_autostart: std::cell::RefCell::new(None),
        }
    }

    pub fn set_on_add_volume(&self, f: impl Fn() + 'static) {
        *self.on_add_volume.borrow_mut() = Some(std::rc::Rc::new(f));
    }

    pub fn set_on_upload_volume(&self, f: impl Fn() + 'static) {
        *self.on_upload_volume.borrow_mut() = Some(std::rc::Rc::new(f));
    }

    pub fn set_on_delete_volume(&self, f: impl Fn(String) + 'static) {
        *self.on_delete_volume.borrow_mut() = Some(std::rc::Rc::new(f));
    }

    pub fn set_on_autostart(&self, f: impl Fn(bool) + 'static) {
        let cb = std::rc::Rc::new(f);
        *self.on_set_autostart.borrow_mut() = Some(cb.clone());
        self.autostart_switch.connect_state_set(move |_, active| {
            cb(active);
            glib::Propagation::Proceed
        });
    }

    pub fn update(
        &self,
        info: &PoolInfo,
        volumes: &[VolumeInfo],
        pool_type: &str,
        pool_path: &str,
    ) {
        self.state_row.set_subtitle(info.state.label());
        self.uuid_row.set_subtitle(&info.uuid);
        self.type_row.set_subtitle(pool_type);
        self.path_row.set_subtitle(pool_path);
        self.autostart_switch.set_active(info.autostart);
        self.persistent_row.set_subtitle(if info.persistent { "Yes" } else { "No" });

        self.capacity_row.set_subtitle(&format_bytes(info.capacity));
        self.allocation_row.set_subtitle(&format_bytes(info.allocation));
        self.available_row.set_subtitle(&format_bytes(info.available));

        let usage_frac = if info.capacity > 0 {
            info.allocation as f64 / info.capacity as f64
        } else {
            0.0
        };
        self.level_bar.set_value(usage_frac.clamp(0.0, 1.0));

        // Button sensitivity
        self.btn_start.set_sensitive(!info.active);
        self.btn_stop.set_sensitive(info.active);
        self.btn_refresh.set_sensitive(info.active);
        self.btn_delete.set_sensitive(true);

        // Remove previously tracked volume rows, then clear the list.
        {
            let rows = self.volume_rows.borrow();
            for row in rows.iter() {
                self.volumes_group.remove(row);
            }
        }
        self.volume_rows.borrow_mut().clear();

        // Clear header suffix.
        self.volumes_group.set_header_suffix(None::<&gtk::Widget>);

        // Header suffix: upload button + add button
        let header_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);

        let upload_vol_btn = gtk::Button::from_icon_name("document-open-symbolic");
        upload_vol_btn.set_tooltip_text(Some("Upload Image to Pool"));
        upload_vol_btn.set_valign(gtk::Align::Center);
        upload_vol_btn.set_sensitive(info.active);
        header_box.append(&upload_vol_btn);

        let add_vol_btn = gtk::Button::from_icon_name("list-add-symbolic");
        add_vol_btn.set_tooltip_text(Some("Create Volume"));
        add_vol_btn.set_valign(gtk::Align::Center);
        add_vol_btn.set_sensitive(info.active);
        header_box.append(&add_vol_btn);

        self.volumes_group.set_header_suffix(Some(&header_box));

        if let Some(ref cb) = *self.on_upload_volume.borrow() {
            let cb = cb.clone();
            upload_vol_btn.connect_clicked(move |_| {
                cb();
            });
        }

        if let Some(ref cb) = *self.on_add_volume.borrow() {
            let cb = cb.clone();
            add_vol_btn.connect_clicked(move |_| {
                cb();
            });
        }

        if volumes.is_empty() {
            let row = adw::ActionRow::new();
            row.set_title("No volumes");
            row.set_activatable(false);
            self.volumes_group.add(&row);
            self.volume_rows.borrow_mut().push(row);
        } else {
            for vol in volumes {
                let row = adw::ActionRow::new();
                row.set_title(&vol.name);
                row.set_subtitle(&format!(
                    "{} ({} / {})",
                    vol.kind,
                    format_bytes(vol.allocation),
                    format_bytes(vol.capacity)
                ));
                row.set_activatable(false);

                let del_btn = gtk::Button::from_icon_name("edit-delete-symbolic");
                del_btn.set_tooltip_text(Some("Delete Volume"));
                del_btn.set_valign(gtk::Align::Center);
                del_btn.add_css_class("flat");
                row.add_suffix(&del_btn);

                let vol_name = vol.name.clone();
                if let Some(ref cb) = *self.on_delete_volume.borrow() {
                    let cb = cb.clone();
                    del_btn.connect_clicked(move |_| {
                        cb(vol_name.clone());
                    });
                }

                self.volumes_group.add(&row);
                self.volume_rows.borrow_mut().push(row);
            }
        }
    }
}
