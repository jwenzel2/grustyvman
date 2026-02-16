use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;

use crate::backend::types::{ForwardMode, NetworkCreateParams};

pub fn show_create_network_dialog(
    parent: &adw::ApplicationWindow,
    on_create: impl Fn(NetworkCreateParams) + 'static,
) {
    let dialog = adw::Window::builder()
        .title("Create Virtual Network")
        .modal(true)
        .transient_for(parent)
        .default_width(500)
        .default_height(500)
        .build();

    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    toolbar.add_top_bar(&header);

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_vexpand(true);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 24);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(24);
    content.set_margin_end(24);

    // --- Network Settings group ---
    let general_group = adw::PreferencesGroup::new();
    general_group.set_title("Network Settings");

    let name_row = adw::EntryRow::new();
    name_row.set_title("Name");
    name_row.set_text("new-network");
    general_group.add(&name_row);

    let mode_labels: Vec<&str> = ForwardMode::ALL.iter().map(|m| m.label()).collect();
    let mode_row = adw::ComboRow::new();
    mode_row.set_title("Forward Mode");
    mode_row.set_model(Some(&gtk::StringList::new(&mode_labels)));
    mode_row.set_selected(0); // NAT
    general_group.add(&mode_row);

    content.append(&general_group);

    // --- Bridge group (only visible for Bridge mode) ---
    let bridge_group = adw::PreferencesGroup::new();
    bridge_group.set_title("Bridge");
    bridge_group.set_visible(false);

    let bridge_name_row = adw::EntryRow::new();
    bridge_name_row.set_title("Bridge Name");
    bridge_name_row.set_text("br0");
    bridge_group.add(&bridge_name_row);

    content.append(&bridge_group);

    // --- IPv4 Configuration group ---
    let ip_group = adw::PreferencesGroup::new();
    ip_group.set_title("IPv4 Configuration");

    let ip_address_row = adw::EntryRow::new();
    ip_address_row.set_title("Network Address");
    ip_address_row.set_text("192.168.100.1");
    ip_group.add(&ip_address_row);

    let netmask_row = adw::EntryRow::new();
    netmask_row.set_title("Netmask");
    netmask_row.set_text("255.255.255.0");
    ip_group.add(&netmask_row);

    content.append(&ip_group);

    // --- DHCP group ---
    let dhcp_group = adw::PreferencesGroup::new();
    dhcp_group.set_title("DHCP");

    let dhcp_switch_row = adw::ActionRow::new();
    dhcp_switch_row.set_title("Enable DHCP");
    dhcp_switch_row.set_activatable(false);
    let dhcp_switch = gtk::Switch::new();
    dhcp_switch.set_valign(gtk::Align::Center);
    dhcp_switch.set_active(true);
    dhcp_switch_row.add_suffix(&dhcp_switch);
    dhcp_group.add(&dhcp_switch_row);

    let dhcp_start_row = adw::EntryRow::new();
    dhcp_start_row.set_title("Range Start");
    dhcp_start_row.set_text("192.168.100.128");
    dhcp_group.add(&dhcp_start_row);

    let dhcp_end_row = adw::EntryRow::new();
    dhcp_end_row.set_title("Range End");
    dhcp_end_row.set_text("192.168.100.254");
    dhcp_group.add(&dhcp_end_row);

    content.append(&dhcp_group);

    // --- DHCP switch toggling range visibility ---
    {
        let dhcp_start_row = dhcp_start_row.clone();
        let dhcp_end_row = dhcp_end_row.clone();
        dhcp_switch.connect_state_set(move |_, active| {
            dhcp_start_row.set_visible(active);
            dhcp_end_row.set_visible(active);
            glib::Propagation::Proceed
        });
    }

    // --- Update visible fields when mode changes ---
    {
        let bridge_group = bridge_group.clone();
        let ip_group = ip_group.clone();
        let dhcp_group = dhcp_group.clone();
        let ip_address_row = ip_address_row.clone();
        let netmask_row = netmask_row.clone();
        let dhcp_start_row = dhcp_start_row.clone();
        let dhcp_end_row = dhcp_end_row.clone();

        mode_row.connect_selected_notify(move |combo| {
            let idx = combo.selected() as usize;
            let mode = ForwardMode::ALL.get(idx).copied().unwrap_or(ForwardMode::Nat);

            match mode {
                ForwardMode::Nat => {
                    bridge_group.set_visible(false);
                    ip_group.set_visible(true);
                    dhcp_group.set_visible(true);
                    ip_address_row.set_text("192.168.100.1");
                    netmask_row.set_text("255.255.255.0");
                    dhcp_start_row.set_text("192.168.100.128");
                    dhcp_end_row.set_text("192.168.100.254");
                }
                ForwardMode::Route => {
                    bridge_group.set_visible(false);
                    ip_group.set_visible(true);
                    dhcp_group.set_visible(true);
                    ip_address_row.set_text("192.168.101.1");
                    netmask_row.set_text("255.255.255.0");
                    dhcp_start_row.set_text("192.168.101.128");
                    dhcp_end_row.set_text("192.168.101.254");
                }
                ForwardMode::Isolated => {
                    bridge_group.set_visible(false);
                    ip_group.set_visible(true);
                    dhcp_group.set_visible(true);
                    ip_address_row.set_text("192.168.102.1");
                    netmask_row.set_text("255.255.255.0");
                    dhcp_start_row.set_text("192.168.102.128");
                    dhcp_end_row.set_text("192.168.102.254");
                }
                ForwardMode::Open => {
                    bridge_group.set_visible(false);
                    ip_group.set_visible(true);
                    dhcp_group.set_visible(true);
                    ip_address_row.set_text("192.168.103.1");
                    netmask_row.set_text("255.255.255.0");
                    dhcp_start_row.set_text("192.168.103.128");
                    dhcp_end_row.set_text("192.168.103.254");
                }
                ForwardMode::Bridge => {
                    bridge_group.set_visible(true);
                    ip_group.set_visible(false);
                    dhcp_group.set_visible(false);
                }
            }
        });
    }

    // --- Buttons ---
    let button_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    button_box.set_halign(gtk::Align::End);
    button_box.set_margin_top(12);

    let cancel_btn = gtk::Button::with_label("Cancel");
    let create_btn = gtk::Button::with_label("Create");
    create_btn.add_css_class("suggested-action");

    button_box.append(&cancel_btn);
    button_box.append(&create_btn);
    content.append(&button_box);

    scrolled.set_child(Some(&content));
    toolbar.set_content(Some(&scrolled));
    dialog.set_content(Some(&toolbar));

    let dialog_weak = dialog.downgrade();
    cancel_btn.connect_clicked(move |_| {
        if let Some(d) = dialog_weak.upgrade() {
            d.close();
        }
    });

    let dialog_weak = dialog.downgrade();
    create_btn.connect_clicked(move |_| {
        let name = name_row.text().to_string();
        if name.is_empty() {
            return;
        }

        let idx = mode_row.selected() as usize;
        let forward_mode = ForwardMode::ALL.get(idx).copied().unwrap_or(ForwardMode::Nat);

        let params = NetworkCreateParams {
            name,
            forward_mode,
            bridge_name: bridge_name_row.text().to_string(),
            ip_address: ip_address_row.text().to_string(),
            ip_netmask: netmask_row.text().to_string(),
            dhcp_enabled: dhcp_switch.is_active(),
            dhcp_start: dhcp_start_row.text().to_string(),
            dhcp_end: dhcp_end_row.text().to_string(),
        };

        on_create(params);
        if let Some(d) = dialog_weak.upgrade() {
            d.close();
        }
    });

    dialog.present();
}
