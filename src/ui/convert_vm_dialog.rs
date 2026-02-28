use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::backend::types::{ConvertNicConfig, DiskFormat, NetworkSourceType};
use crate::backend::virtualbox::{ConvertVBoxParams, VBoxConfig};
use crate::backend::vmware::{ConvertVmParams, VmwareConfig};

pub fn show_convert_vm_file_dialog(
    parent: &adw::ApplicationWindow,
    pool_names: Vec<String>,
    network_names: Vec<String>,
    on_convert: impl Fn(ConvertVmParams) + 'static,
) {
    let file_dialog = gtk::FileDialog::new();
    file_dialog.set_title("Select VMware VM File");

    let filter = gtk::FileFilter::new();
    filter.add_pattern("*.vmx");
    filter.add_pattern("*.ova");
    filter.add_pattern("*.ovf");
    filter.set_name(Some("VMware Files (*.vmx, *.ova, *.ovf)"));

    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    file_dialog.set_filters(Some(&filters));

    let parent_clone = parent.clone();
    let on_convert = Rc::new(on_convert);

    file_dialog.open(Some(parent), gio::Cancellable::NONE, move |result| {
        let Ok(file) = result else { return };
        let Some(path) = file.path() else { return };
        let path_str = path.to_string_lossy().to_string();

        let config = match path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref()
        {
            Some("vmx") => crate::backend::vmware::parse_vmx(&path_str),
            Some("ovf") => crate::backend::vmware::parse_ovf(&path_str),
            Some("ova") => {
                match crate::backend::vmware::extract_ova(&path_str) {
                    Ok((config, _tmp_dir)) => Ok(config),
                    Err(e) => Err(e),
                }
            }
            _ => return,
        };

        match config {
            Ok(config) => {
                let cb = on_convert.clone();
                show_convert_config_dialog(
                    &parent_clone,
                    config,
                    pool_names.clone(),
                    network_names.clone(),
                    move |params| cb(params),
                );
            }
            Err(e) => {
                let toast = adw::Toast::new(&format!("Failed to parse VMware file: {e}"));
                // Try to find a toast overlay - if parent has one
                if let Some(child) = parent_clone.content() {
                    if let Some(overlay) = child.downcast_ref::<adw::ToastOverlay>() {
                        overlay.add_toast(toast);
                    }
                }
            }
        }
    });
}

fn show_convert_config_dialog(
    parent: &adw::ApplicationWindow,
    config: VmwareConfig,
    pool_names: Vec<String>,
    network_names: Vec<String>,
    on_convert: impl Fn(ConvertVmParams) + 'static,
) {
    let dialog = gtk::Window::new();
    dialog.set_title(Some("Import VMware VM"));
    dialog.set_default_size(480, 640);
    dialog.set_decorated(false);
    dialog.set_modal(true);
    dialog.set_transient_for(Some(parent));

    let toolbar_view = adw::ToolbarView::new();

    let header = adw::HeaderBar::new();
    toolbar_view.add_top_bar(&header);

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_propagate_natural_height(true);
    scrolled.set_max_content_height(600);

    let clamp = adw::Clamp::new();
    clamp.set_maximum_size(480);
    clamp.set_margin_top(24);
    clamp.set_margin_bottom(24);
    clamp.set_margin_start(12);
    clamp.set_margin_end(12);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 24);

    // --- General group ---
    let general_group = adw::PreferencesGroup::new();
    general_group.set_title("General");

    let name_row = adw::EntryRow::new();
    name_row.set_title("Name");
    name_row.set_text(&config.name);
    general_group.add(&name_row);

    let firmware_row = adw::ActionRow::new();
    firmware_row.set_title("Firmware");
    firmware_row.set_subtitle(config.firmware.label());
    general_group.add(&firmware_row);

    if let Some(ref os) = config.guest_os {
        let os_row = adw::ActionRow::new();
        os_row.set_title("Guest OS");
        os_row.set_subtitle(os);
        general_group.add(&os_row);
    }

    content.append(&general_group);

    // --- Resources group ---
    let resources_group = adw::PreferencesGroup::new();
    resources_group.set_title("Resources");

    let cpu_row = adw::SpinRow::with_range(1.0, 64.0, 1.0);
    cpu_row.set_title("vCPUs");
    cpu_row.set_value(config.vcpus as f64);
    resources_group.add(&cpu_row);

    let memory_row = adw::SpinRow::with_range(256.0, 131072.0, 256.0);
    memory_row.set_title("Memory (MiB)");
    memory_row.set_value(config.memory_mib as f64);
    resources_group.add(&memory_row);

    content.append(&resources_group);

    // --- Disks group ---
    let disks_group = adw::PreferencesGroup::new();
    disks_group.set_title("Disks");

    for disk in &config.disks {
        let fname = std::path::Path::new(&disk.vmdk_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let row = adw::ActionRow::new();
        row.set_title(&fname);
        row.set_subtitle(&format!("Bus: {}", disk.bus_type));
        disks_group.add(&row);
    }

    let format_labels: Vec<&str> = DiskFormat::ALL.iter().map(|f| f.label()).collect();
    let format_list = gtk::StringList::new(&format_labels);
    let format_row = adw::ComboRow::new();
    format_row.set_title("Target Format");
    format_row.set_model(Some(&format_list));
    format_row.set_selected(0); // qcow2 default
    disks_group.add(&format_row);

    content.append(&disks_group);

    // --- Storage group ---
    let storage_group = adw::PreferencesGroup::new();
    storage_group.set_title("Storage");

    let pool_labels: Vec<&str> = pool_names.iter().map(|s| s.as_str()).collect();
    let pool_list = gtk::StringList::new(&pool_labels);
    let pool_row = adw::ComboRow::new();
    pool_row.set_title("Target Pool");
    pool_row.set_model(Some(&pool_list));
    pool_row.set_selected(0);
    storage_group.add(&pool_row);

    content.append(&storage_group);

    // --- Network Adapters group ---
    // We store NIC widgets for later retrieval
    struct NicWidgets {
        type_row: adw::ComboRow,
        _value_stack: gtk::Stack,
        network_combo: adw::ComboRow,
        device_entry: adw::EntryRow,
        mac: Option<String>,
    }

    let nic_widgets: Rc<RefCell<Vec<NicWidgets>>> = Rc::new(RefCell::new(Vec::new()));

    if !config.nics.is_empty() {
        let net_group = adw::PreferencesGroup::new();
        net_group.set_title("Network Adapters");

        for (i, nic) in config.nics.iter().enumerate() {
            // MAC address (read-only)
            if let Some(ref mac) = nic.mac_address {
                let mac_row = adw::ActionRow::new();
                mac_row.set_title(&format!("NIC {} MAC", i + 1));
                mac_row.set_subtitle(mac);
                net_group.add(&mac_row);
            }

            // Source type combo
            let type_labels: Vec<&str> =
                NetworkSourceType::ALL.iter().map(|t| t.label()).collect();
            let type_list = gtk::StringList::new(&type_labels);
            let type_row = adw::ComboRow::new();
            type_row.set_title(&format!("NIC {} Source Type", i + 1));
            type_row.set_model(Some(&type_list));
            type_row.set_selected(0);
            net_group.add(&type_row);

            // Value: either a combo for virtual networks or an entry for bridge/device name
            let value_stack = gtk::Stack::new();

            let net_labels: Vec<&str> = network_names.iter().map(|s| s.as_str()).collect();
            let net_list = gtk::StringList::new(&net_labels);
            let network_combo = adw::ComboRow::new();
            network_combo.set_title(&format!("NIC {} Network", i + 1));
            network_combo.set_model(Some(&net_list));
            if !network_names.is_empty() {
                network_combo.set_selected(0);
            }
            value_stack.add_named(&network_combo, Some("network"));

            let device_entry = adw::EntryRow::new();
            device_entry.set_title(&format!("NIC {} Device", i + 1));
            value_stack.add_named(&device_entry, Some("device"));

            value_stack.set_visible_child_name("network");
            net_group.add(&value_stack);

            // Switch value widget based on type selection
            let stack = value_stack.clone();
            type_row.connect_selected_notify(move |row| {
                let idx = row.selected();
                match NetworkSourceType::ALL.get(idx as usize) {
                    Some(NetworkSourceType::VirtualNetwork) => {
                        stack.set_visible_child_name("network");
                    }
                    _ => {
                        stack.set_visible_child_name("device");
                    }
                }
            });

            nic_widgets.borrow_mut().push(NicWidgets {
                type_row,
                _value_stack: value_stack,
                network_combo,
                device_entry,
                mac: nic.mac_address.clone(),
            });
        }

        content.append(&net_group);
    }

    // --- Convert button ---
    let convert_btn = gtk::Button::with_label("Convert");
    convert_btn.add_css_class("suggested-action");
    convert_btn.add_css_class("pill");
    convert_btn.set_halign(gtk::Align::Center);
    convert_btn.set_margin_top(12);
    content.append(&convert_btn);

    clamp.set_child(Some(&content));
    scrolled.set_child(Some(&clamp));
    toolbar_view.set_content(Some(&scrolled));
    dialog.set_child(Some(&toolbar_view));

    let dialog_weak = dialog.downgrade();
    let config = Rc::new(config);
    let pool_names = Rc::new(pool_names);
    let network_names = Rc::new(network_names);
    let on_convert = Rc::new(on_convert);

    convert_btn.connect_clicked(move |_| {
        let target_name = name_row.text().to_string();
        if target_name.is_empty() {
            return;
        }

        let format_idx = format_row.selected() as usize;
        let disk_format = DiskFormat::ALL
            .get(format_idx)
            .copied()
            .unwrap_or(DiskFormat::Qcow2);

        let pool_idx = pool_row.selected() as usize;
        let pool_name = pool_names
            .get(pool_idx)
            .cloned()
            .unwrap_or_else(|| "default".to_string());

        // Gather NIC configs
        let mut nic_configs = Vec::new();
        for w in nic_widgets.borrow().iter() {
            let type_idx = w.type_row.selected() as usize;
            let source_type = NetworkSourceType::ALL
                .get(type_idx)
                .copied()
                .unwrap_or(NetworkSourceType::VirtualNetwork);

            let source_value = match source_type {
                NetworkSourceType::VirtualNetwork => {
                    let idx = w.network_combo.selected() as usize;
                    network_names
                        .get(idx)
                        .cloned()
                        .unwrap_or_else(|| "default".to_string())
                }
                _ => w.device_entry.text().to_string(),
            };

            nic_configs.push(ConvertNicConfig {
                source_type,
                source_value,
                mac_address: w.mac.clone(),
            });
        }

        let params = ConvertVmParams {
            vmware_config: (*config).clone(),
            target_name,
            disk_format,
            pool_name,
            nic_configs,
        };

        if let Some(d) = dialog_weak.upgrade() {
            d.close();
        }

        on_convert(params);
    });

    dialog.present();
}

// ---------------------------------------------------------------------------
// VirtualBox import dialogs
// ---------------------------------------------------------------------------

pub fn show_convert_vbox_file_dialog(
    parent: &adw::ApplicationWindow,
    pool_names: Vec<String>,
    network_names: Vec<String>,
    on_convert: impl Fn(ConvertVBoxParams) + 'static,
) {
    let file_dialog = gtk::FileDialog::new();
    file_dialog.set_title("Select VirtualBox VM File");

    let filter = gtk::FileFilter::new();
    filter.add_pattern("*.vbox");
    filter.set_name(Some("VirtualBox Files (*.vbox)"));

    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    file_dialog.set_filters(Some(&filters));

    let parent_clone = parent.clone();
    let on_convert = Rc::new(on_convert);

    file_dialog.open(Some(parent), gio::Cancellable::NONE, move |result| {
        let Ok(file) = result else { return };
        let Some(path) = file.path() else { return };
        let path_str = path.to_string_lossy().to_string();

        match crate::backend::virtualbox::parse_vbox(&path_str) {
            Ok(config) => {
                let cb = on_convert.clone();
                show_convert_vbox_config_dialog(
                    &parent_clone,
                    config,
                    pool_names.clone(),
                    network_names.clone(),
                    move |params| cb(params),
                );
            }
            Err(e) => {
                let toast = adw::Toast::new(&format!("Failed to parse VirtualBox file: {e}"));
                if let Some(child) = parent_clone.content() {
                    if let Some(overlay) = child.downcast_ref::<adw::ToastOverlay>() {
                        overlay.add_toast(toast);
                    }
                }
            }
        }
    });
}

fn show_convert_vbox_config_dialog(
    parent: &adw::ApplicationWindow,
    config: VBoxConfig,
    pool_names: Vec<String>,
    network_names: Vec<String>,
    on_convert: impl Fn(ConvertVBoxParams) + 'static,
) {
    let dialog = gtk::Window::new();
    dialog.set_title(Some("Import VirtualBox VM"));
    dialog.set_default_size(480, 640);
    dialog.set_decorated(false);
    dialog.set_modal(true);
    dialog.set_transient_for(Some(parent));

    let toolbar_view = adw::ToolbarView::new();

    let header = adw::HeaderBar::new();
    toolbar_view.add_top_bar(&header);

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_propagate_natural_height(true);
    scrolled.set_max_content_height(600);

    let clamp = adw::Clamp::new();
    clamp.set_maximum_size(480);
    clamp.set_margin_top(24);
    clamp.set_margin_bottom(24);
    clamp.set_margin_start(12);
    clamp.set_margin_end(12);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 24);

    // --- General group ---
    let general_group = adw::PreferencesGroup::new();
    general_group.set_title("General");

    let name_row = adw::EntryRow::new();
    name_row.set_title("Name");
    name_row.set_text(&config.name);
    general_group.add(&name_row);

    let firmware_row = adw::ActionRow::new();
    firmware_row.set_title("Firmware");
    firmware_row.set_subtitle(config.firmware.label());
    general_group.add(&firmware_row);

    if let Some(ref os) = config.guest_os {
        let os_row = adw::ActionRow::new();
        os_row.set_title("Guest OS");
        os_row.set_subtitle(os);
        general_group.add(&os_row);
    }

    content.append(&general_group);

    // --- Resources group ---
    let resources_group = adw::PreferencesGroup::new();
    resources_group.set_title("Resources");

    let cpu_row = adw::SpinRow::with_range(1.0, 64.0, 1.0);
    cpu_row.set_title("vCPUs");
    cpu_row.set_value(config.vcpus as f64);
    resources_group.add(&cpu_row);

    let memory_row = adw::SpinRow::with_range(256.0, 131072.0, 256.0);
    memory_row.set_title("Memory (MiB)");
    memory_row.set_value(config.memory_mib as f64);
    resources_group.add(&memory_row);

    content.append(&resources_group);

    // --- Disks group ---
    let disks_group = adw::PreferencesGroup::new();
    disks_group.set_title("Disks");

    for disk in &config.disks {
        let fname = std::path::Path::new(&disk.source_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let row = adw::ActionRow::new();
        row.set_title(&fname);
        row.set_subtitle(&format!("{} | Bus: {}", disk.format.to_uppercase(), disk.bus_type));
        disks_group.add(&row);
    }

    let format_labels: Vec<&str> = DiskFormat::ALL.iter().map(|f| f.label()).collect();
    let format_list = gtk::StringList::new(&format_labels);
    let format_row = adw::ComboRow::new();
    format_row.set_title("Target Format");
    format_row.set_model(Some(&format_list));
    format_row.set_selected(0);
    disks_group.add(&format_row);

    content.append(&disks_group);

    // --- Storage group ---
    let storage_group = adw::PreferencesGroup::new();
    storage_group.set_title("Storage");

    let pool_labels: Vec<&str> = pool_names.iter().map(|s| s.as_str()).collect();
    let pool_list = gtk::StringList::new(&pool_labels);
    let pool_row = adw::ComboRow::new();
    pool_row.set_title("Target Pool");
    pool_row.set_model(Some(&pool_list));
    pool_row.set_selected(0);
    storage_group.add(&pool_row);

    content.append(&storage_group);

    // --- Network Adapters group ---
    struct NicWidgets {
        type_row: adw::ComboRow,
        _value_stack: gtk::Stack,
        network_combo: adw::ComboRow,
        device_entry: adw::EntryRow,
        mac: Option<String>,
    }

    let nic_widgets: Rc<RefCell<Vec<NicWidgets>>> = Rc::new(RefCell::new(Vec::new()));

    if !config.nics.is_empty() {
        let net_group = adw::PreferencesGroup::new();
        net_group.set_title("Network Adapters");

        for (i, nic) in config.nics.iter().enumerate() {
            if let Some(ref mac) = nic.mac_address {
                let mac_row = adw::ActionRow::new();
                mac_row.set_title(&format!("NIC {} MAC", i + 1));
                mac_row.set_subtitle(mac);
                net_group.add(&mac_row);
            }

            let type_labels: Vec<&str> =
                NetworkSourceType::ALL.iter().map(|t| t.label()).collect();
            let type_list = gtk::StringList::new(&type_labels);
            let type_row = adw::ComboRow::new();
            type_row.set_title(&format!("NIC {} Source Type", i + 1));
            type_row.set_model(Some(&type_list));
            type_row.set_selected(0);
            net_group.add(&type_row);

            let value_stack = gtk::Stack::new();

            let net_labels: Vec<&str> = network_names.iter().map(|s| s.as_str()).collect();
            let net_list = gtk::StringList::new(&net_labels);
            let network_combo = adw::ComboRow::new();
            network_combo.set_title(&format!("NIC {} Network", i + 1));
            network_combo.set_model(Some(&net_list));
            if !network_names.is_empty() {
                network_combo.set_selected(0);
            }
            value_stack.add_named(&network_combo, Some("network"));

            let device_entry = adw::EntryRow::new();
            device_entry.set_title(&format!("NIC {} Device", i + 1));
            value_stack.add_named(&device_entry, Some("device"));

            value_stack.set_visible_child_name("network");
            net_group.add(&value_stack);

            let stack = value_stack.clone();
            type_row.connect_selected_notify(move |row| {
                let idx = row.selected();
                match NetworkSourceType::ALL.get(idx as usize) {
                    Some(NetworkSourceType::VirtualNetwork) => {
                        stack.set_visible_child_name("network");
                    }
                    _ => {
                        stack.set_visible_child_name("device");
                    }
                }
            });

            nic_widgets.borrow_mut().push(NicWidgets {
                type_row,
                _value_stack: value_stack,
                network_combo,
                device_entry,
                mac: nic.mac_address.clone(),
            });
        }

        content.append(&net_group);
    }

    // --- Convert button ---
    let convert_btn = gtk::Button::with_label("Convert");
    convert_btn.add_css_class("suggested-action");
    convert_btn.add_css_class("pill");
    convert_btn.set_halign(gtk::Align::Center);
    convert_btn.set_margin_top(12);
    content.append(&convert_btn);

    clamp.set_child(Some(&content));
    scrolled.set_child(Some(&clamp));
    toolbar_view.set_content(Some(&scrolled));
    dialog.set_child(Some(&toolbar_view));

    let dialog_weak = dialog.downgrade();
    let config = Rc::new(config);
    let pool_names = Rc::new(pool_names);
    let network_names = Rc::new(network_names);
    let on_convert = Rc::new(on_convert);

    convert_btn.connect_clicked(move |_| {
        let target_name = name_row.text().to_string();
        if target_name.is_empty() {
            return;
        }

        let format_idx = format_row.selected() as usize;
        let disk_format = DiskFormat::ALL
            .get(format_idx)
            .copied()
            .unwrap_or(DiskFormat::Qcow2);

        let pool_idx = pool_row.selected() as usize;
        let pool_name = pool_names
            .get(pool_idx)
            .cloned()
            .unwrap_or_else(|| "default".to_string());

        let mut nic_configs = Vec::new();
        for w in nic_widgets.borrow().iter() {
            let type_idx = w.type_row.selected() as usize;
            let source_type = NetworkSourceType::ALL
                .get(type_idx)
                .copied()
                .unwrap_or(NetworkSourceType::VirtualNetwork);

            let source_value = match source_type {
                NetworkSourceType::VirtualNetwork => {
                    let idx = w.network_combo.selected() as usize;
                    network_names
                        .get(idx)
                        .cloned()
                        .unwrap_or_else(|| "default".to_string())
                }
                _ => w.device_entry.text().to_string(),
            };

            nic_configs.push(ConvertNicConfig {
                source_type,
                source_value,
                mac_address: w.mac.clone(),
            });
        }

        let params = ConvertVBoxParams {
            vbox_config: (*config).clone(),
            target_name,
            disk_format,
            pool_name,
            nic_configs,
        };

        if let Some(d) = dialog_weak.upgrade() {
            d.close();
        }

        on_convert(params);
    });

    dialog.present();
}
