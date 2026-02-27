use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;

use crate::backend::types::VirtNetworkInfo;

pub struct NetworkDetailsView {
    pub container: gtk::Box,
    state_row: adw::ActionRow,
    uuid_row: adw::ActionRow,
    forward_mode_row: adw::ActionRow,
    bridge_name_row: adw::ActionRow,
    persistent_row: adw::ActionRow,
    autostart_switch: gtk::Switch,
    #[allow(dead_code)]
    autostart_row: adw::ActionRow,
    ip_group: adw::PreferencesGroup,
    ip_address_row: adw::ActionRow,
    ip_netmask_row: adw::ActionRow,
    dhcp_group: adw::PreferencesGroup,
    dhcp_start_row: adw::ActionRow,
    dhcp_end_row: adw::ActionRow,
    pub btn_start: gtk::Button,
    pub btn_stop: gtk::Button,
    pub btn_delete: gtk::Button,
    on_set_autostart: std::cell::RefCell<Option<std::rc::Rc<dyn Fn(bool)>>>,
}

impl NetworkDetailsView {
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
        btn_start.set_tooltip_text(Some("Start Network"));

        let btn_stop = gtk::Button::new();
        btn_stop.set_icon_name("media-playback-stop-symbolic");
        btn_stop.set_tooltip_text(Some("Stop Network"));

        let btn_delete = gtk::Button::new();
        btn_delete.set_icon_name("user-trash-symbolic");
        btn_delete.set_tooltip_text(Some("Delete Network"));
        btn_delete.add_css_class("destructive-action");

        actions_box.append(&btn_start);
        actions_box.append(&btn_stop);
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

        let forward_mode_row = adw::ActionRow::new();
        forward_mode_row.set_title("Forward Mode");
        forward_mode_row.set_activatable(false);
        status_group.add(&forward_mode_row);

        let bridge_name_row = adw::ActionRow::new();
        bridge_name_row.set_title("Bridge Name");
        bridge_name_row.set_activatable(false);
        status_group.add(&bridge_name_row);

        let persistent_row = adw::ActionRow::new();
        persistent_row.set_title("Persistent");
        persistent_row.set_activatable(false);
        status_group.add(&persistent_row);

        let autostart_row = adw::ActionRow::new();
        autostart_row.set_title("Autostart");
        autostart_row.set_activatable(false);
        let autostart_switch = gtk::Switch::new();
        autostart_switch.set_valign(gtk::Align::Center);
        autostart_row.add_suffix(&autostart_switch);
        status_group.add(&autostart_row);

        container.append(&status_group);

        // IP Configuration group
        let ip_group = adw::PreferencesGroup::new();
        ip_group.set_title("IP Configuration");

        let ip_address_row = adw::ActionRow::new();
        ip_address_row.set_title("IP Address");
        ip_address_row.set_activatable(false);
        ip_group.add(&ip_address_row);

        let ip_netmask_row = adw::ActionRow::new();
        ip_netmask_row.set_title("Netmask");
        ip_netmask_row.set_activatable(false);
        ip_group.add(&ip_netmask_row);

        container.append(&ip_group);

        // DHCP group
        let dhcp_group = adw::PreferencesGroup::new();
        dhcp_group.set_title("DHCP");

        let dhcp_start_row = adw::ActionRow::new();
        dhcp_start_row.set_title("Range Start");
        dhcp_start_row.set_activatable(false);
        dhcp_group.add(&dhcp_start_row);

        let dhcp_end_row = adw::ActionRow::new();
        dhcp_end_row.set_title("Range End");
        dhcp_end_row.set_activatable(false);
        dhcp_group.add(&dhcp_end_row);

        container.append(&dhcp_group);

        Self {
            container,
            state_row,
            uuid_row,
            forward_mode_row,
            bridge_name_row,
            persistent_row,
            autostart_switch,
            autostart_row,
            ip_group,
            ip_address_row,
            ip_netmask_row,
            dhcp_group,
            dhcp_start_row,
            dhcp_end_row,
            btn_start,
            btn_stop,
            btn_delete,
            on_set_autostart: std::cell::RefCell::new(None),
        }
    }

    pub fn set_on_autostart(&self, f: impl Fn(bool) + 'static) {
        let cb = std::rc::Rc::new(f);
        *self.on_set_autostart.borrow_mut() = Some(cb.clone());
        self.autostart_switch.connect_state_set(move |_, active| {
            cb(active);
            glib::Propagation::Proceed
        });
    }

    pub fn update(&self, info: &VirtNetworkInfo) {
        self.state_row.set_subtitle(info.state.label());
        self.uuid_row.set_subtitle(&info.uuid);
        self.forward_mode_row.set_subtitle(info.forward_mode.label());
        self.bridge_name_row.set_subtitle(
            info.bridge_name.as_deref().unwrap_or("None"),
        );
        self.persistent_row.set_subtitle(if info.persistent { "Yes" } else { "No" });
        self.autostart_switch.set_active(info.autostart);

        // Button sensitivity
        self.btn_start.set_sensitive(!info.active);
        self.btn_stop.set_sensitive(info.active);
        self.btn_delete.set_sensitive(true);

        // IP Configuration visibility
        let has_ip = info.ip_address.is_some();
        self.ip_group.set_visible(has_ip);
        if let Some(ref addr) = info.ip_address {
            self.ip_address_row.set_subtitle(addr);
        }
        if let Some(ref mask) = info.ip_netmask {
            self.ip_netmask_row.set_subtitle(mask);
        }

        // DHCP visibility
        let has_dhcp = info.dhcp_start.is_some();
        self.dhcp_group.set_visible(has_dhcp);
        if let Some(ref start) = info.dhcp_start {
            self.dhcp_start_row.set_subtitle(start);
        }
        if let Some(ref end) = info.dhcp_end {
            self.dhcp_end_row.set_subtitle(end);
        }
    }
}
