use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;

use crate::backend::types::NewNetworkParams;

pub fn show_add_network_dialog(
    parent: &adw::ApplicationWindow,
    available_networks: &[String],
    on_add: impl Fn(NewNetworkParams) + 'static,
) {
    let dialog = gtk::Window::new();
    dialog.set_title(Some("Add Network Interface"));
    dialog.set_default_size(400, 320);
    dialog.set_modal(true);
    dialog.set_transient_for(Some(parent));

    let toolbar_view = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    toolbar_view.add_top_bar(&header);

    let clamp = adw::Clamp::new();
    clamp.set_maximum_size(400);
    clamp.set_margin_top(24);
    clamp.set_margin_bottom(24);
    clamp.set_margin_start(12);
    clamp.set_margin_end(12);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 24);

    let group = adw::PreferencesGroup::new();
    group.set_title("Network Interface");

    // Network selection
    let net_labels: Vec<&str> = if available_networks.is_empty() {
        vec!["default"]
    } else {
        available_networks.iter().map(|s| s.as_str()).collect()
    };
    let net_list = gtk::StringList::new(&net_labels);
    let net_row = adw::ComboRow::new();
    net_row.set_title("Network");
    net_row.set_model(Some(&net_list));
    group.add(&net_row);

    // Model selection
    let model_list = gtk::StringList::new(&["virtio", "e1000", "rtl8139"]);
    let model_row = adw::ComboRow::new();
    model_row.set_title("Model");
    model_row.set_model(Some(&model_list));
    group.add(&model_row);

    // Optional MAC address
    let mac_row = adw::EntryRow::new();
    mac_row.set_title("MAC Address (optional)");
    mac_row.set_show_apply_button(false);
    group.add(&mac_row);

    content.append(&group);

    // Add button
    let add_btn = gtk::Button::with_label("Add Network Interface");
    add_btn.add_css_class("suggested-action");
    add_btn.add_css_class("pill");
    add_btn.set_halign(gtk::Align::Center);
    add_btn.set_margin_top(12);
    content.append(&add_btn);

    clamp.set_child(Some(&content));
    toolbar_view.set_content(Some(&clamp));
    dialog.set_child(Some(&toolbar_view));

    let dialog_ref = dialog.clone();
    let networks_owned: Vec<String> = if available_networks.is_empty() {
        vec!["default".to_string()]
    } else {
        available_networks.to_vec()
    };

    add_btn.connect_clicked(move |_| {
        let net_idx = net_row.selected() as usize;
        let source_network = networks_owned
            .get(net_idx)
            .cloned()
            .unwrap_or_else(|| "default".to_string());

        let model_idx = model_row.selected() as usize;
        let model_type = ["virtio", "e1000", "rtl8139"][model_idx].to_string();

        let mac_text = mac_row.text().trim().to_string();
        let mac_address = if mac_text.is_empty() { None } else { Some(mac_text) };

        let params = NewNetworkParams {
            source_network,
            model_type,
            mac_address,
        };

        on_add(params);
        dialog_ref.close();
    });

    dialog.present();
}
