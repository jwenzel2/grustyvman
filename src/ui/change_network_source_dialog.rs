use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;

use crate::backend::types::{ChangeNetworkSourceParams, NetworkSourceType};

pub fn show_change_network_source_dialog(
    parent: &adw::ApplicationWindow,
    available_networks: &[String],
    current_interface_type: &str,
    on_change: impl Fn(ChangeNetworkSourceParams) + 'static,
) {
    let dialog = gtk::Window::new();
    dialog.set_title(Some("Change Network Source"));
    dialog.set_default_size(420, -1);
    dialog.set_modal(true);
    dialog.set_transient_for(Some(parent));

    let toolbar_view = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    toolbar_view.add_top_bar(&header);

    let clamp = adw::Clamp::new();
    clamp.set_maximum_size(420);
    clamp.set_margin_top(24);
    clamp.set_margin_bottom(24);
    clamp.set_margin_start(12);
    clamp.set_margin_end(12);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 16);

    let group = adw::PreferencesGroup::new();
    group.set_title("Network Source");

    // Source type combo
    let type_labels: Vec<&str> = NetworkSourceType::ALL.iter().map(|t| t.label()).collect();
    let type_list = gtk::StringList::new(&type_labels);
    let type_row = adw::ComboRow::new();
    type_row.set_title("Source Type");
    type_row.set_model(Some(&type_list));

    // Pre-select based on current interface type
    let initial_idx = match current_interface_type {
        "bridge" => 1,
        "direct" => 2,
        "vdpa" => 3,
        _ => 0,
    };
    type_row.set_selected(initial_idx);
    group.add(&type_row);

    // Virtual network combo (visible when VirtualNetwork selected)
    let net_labels: Vec<&str> = if available_networks.is_empty() {
        vec!["default"]
    } else {
        available_networks.iter().map(|s| s.as_str()).collect()
    };
    let net_list = gtk::StringList::new(&net_labels);
    let net_row = adw::ComboRow::new();
    net_row.set_title("Virtual Network");
    net_row.set_model(Some(&net_list));
    net_row.set_visible(initial_idx == 0);
    group.add(&net_row);

    // Device name entry (visible for bridge / macvtap / vdpa)
    let dev_row = adw::EntryRow::new();
    dev_row.set_title("Device Name");
    dev_row.set_visible(initial_idx != 0);
    group.add(&dev_row);

    // Update visibility when type changes
    let net_row_ref = net_row.clone();
    let dev_row_ref = dev_row.clone();
    type_row.connect_selected_notify(move |row| {
        let is_vnet = row.selected() == 0;
        net_row_ref.set_visible(is_vnet);
        dev_row_ref.set_visible(!is_vnet);
    });

    content.append(&group);

    let apply_btn = gtk::Button::with_label("Apply");
    apply_btn.add_css_class("suggested-action");
    apply_btn.add_css_class("pill");
    apply_btn.set_halign(gtk::Align::Center);
    apply_btn.set_margin_top(8);
    content.append(&apply_btn);

    clamp.set_child(Some(&content));
    toolbar_view.set_content(Some(&clamp));
    dialog.set_child(Some(&toolbar_view));

    let networks_owned: Vec<String> = if available_networks.is_empty() {
        vec!["default".to_string()]
    } else {
        available_networks.to_vec()
    };

    let dialog_ref = dialog.clone();
    apply_btn.connect_clicked(move |_| {
        let type_idx = type_row.selected() as usize;
        let source_type = NetworkSourceType::ALL
            .get(type_idx)
            .copied()
            .unwrap_or(NetworkSourceType::VirtualNetwork);

        let value = match source_type {
            NetworkSourceType::VirtualNetwork => {
                let idx = net_row.selected() as usize;
                networks_owned
                    .get(idx)
                    .cloned()
                    .unwrap_or_else(|| "default".to_string())
            }
            _ => {
                let v = dev_row.text().trim().to_string();
                if v.is_empty() {
                    return;
                }
                v
            }
        };

        on_change(ChangeNetworkSourceParams { source_type, value });
        dialog_ref.close();
    });

    dialog.present();
}
