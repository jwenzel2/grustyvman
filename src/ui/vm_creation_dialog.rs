use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::backend::domain_xml::NewVmParams;
use crate::backend::types::{DiskFormat, FirmwareType, NetworkModel, NetworkSourceType, NewVmNetworkConfig, VolumeInfo};

pub fn show_creation_dialog(
    parent: &adw::ApplicationWindow,
    pool_volumes: Vec<(String, Vec<VolumeInfo>)>,
    virtual_networks: Vec<String>,
    on_create: impl Fn(NewVmParams) + 'static,
) {
    let dialog = gtk::Window::new();
    dialog.set_title(Some("New Virtual Machine"));
    dialog.set_default_size(480, 520);
    dialog.set_decorated(false); // suppress WM title bar; adw::HeaderBar provides the only bar
    dialog.set_modal(true);
    dialog.set_transient_for(Some(parent));

    let toolbar_view = adw::ToolbarView::new();

    let header = adw::HeaderBar::new();
    toolbar_view.add_top_bar(&header);

    let clamp = adw::Clamp::new();
    clamp.set_maximum_size(480);
    clamp.set_margin_top(24);
    clamp.set_margin_bottom(24);
    clamp.set_margin_start(12);
    clamp.set_margin_end(12);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 24);

    // General group
    let general_group = adw::PreferencesGroup::new();
    general_group.set_title("General");

    let name_row = adw::EntryRow::new();
    name_row.set_title("Name");
    name_row.set_text("new-vm");
    general_group.add(&name_row);

    let firmware_labels: Vec<&str> = FirmwareType::ALL.iter().map(|f| f.label()).collect();
    let firmware_list = gtk::StringList::new(&firmware_labels);
    let firmware_row = adw::ComboRow::new();
    firmware_row.set_title("Firmware");
    firmware_row.set_model(Some(&firmware_list));
    firmware_row.set_selected(0);
    general_group.add(&firmware_row);

    content.append(&general_group);

    // Resources group
    let resources_group = adw::PreferencesGroup::new();
    resources_group.set_title("Resources");

    let cpu_row = adw::SpinRow::with_range(1.0, 32.0, 1.0);
    cpu_row.set_title("vCPUs");
    cpu_row.set_value(2.0);
    resources_group.add(&cpu_row);

    let memory_row = adw::SpinRow::with_range(256.0, 65536.0, 256.0);
    memory_row.set_title("Memory (MiB)");
    memory_row.set_value(2048.0);
    resources_group.add(&memory_row);

    let disk_row = adw::SpinRow::with_range(1.0, 1000.0, 1.0);
    disk_row.set_title("Disk Size (GiB)");
    disk_row.set_value(20.0);
    resources_group.add(&disk_row);

    let format_labels: Vec<&str> = DiskFormat::ALL.iter().map(|f| f.label()).collect();
    let format_list = gtk::StringList::new(&format_labels);
    let format_row = adw::ComboRow::new();
    format_row.set_title("Disk Format");
    format_row.set_model(Some(&format_list));
    format_row.set_selected(0); // qcow2 by default
    resources_group.add(&format_row);

    content.append(&resources_group);

    // ISO group
    let iso_group = adw::PreferencesGroup::new();
    iso_group.set_title("Installation Media");

    let iso_row = adw::ActionRow::new();
    iso_row.set_title("ISO Image");
    iso_row.set_subtitle("No ISO selected");

    let iso_path: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    let browse_btn = gtk::Button::with_label("Browse...");
    browse_btn.set_valign(gtk::Align::Center);
    if pool_volumes.is_empty() {
        browse_btn.set_sensitive(false);
        browse_btn.set_tooltip_text(Some("No storage pools available"));
    }
    iso_row.add_suffix(&browse_btn);

    let clear_btn = gtk::Button::from_icon_name("edit-clear-symbolic");
    clear_btn.set_valign(gtk::Align::Center);
    clear_btn.set_tooltip_text(Some("Clear ISO selection"));
    clear_btn.set_visible(false);
    iso_row.add_suffix(&clear_btn);

    iso_group.add(&iso_row);
    content.append(&iso_group);

    // Network group
    let network_group = adw::PreferencesGroup::new();
    network_group.set_title("Network");

    // Source type
    let src_labels: Vec<&str> = NetworkSourceType::ALL.iter().map(|s| s.label()).collect();
    let src_list = gtk::StringList::new(&src_labels);
    let src_type_row = adw::ComboRow::new();
    src_type_row.set_title("Network Source");
    src_type_row.set_model(Some(&src_list));
    src_type_row.set_selected(0); // Virtual Network
    network_group.add(&src_type_row);

    // Virtual network picker row (visible when source = VirtualNetwork)
    let virt_net_labels: Vec<&str> = virtual_networks.iter().map(|s| s.as_str()).collect();
    let virt_net_row = adw::ComboRow::new();
    virt_net_row.set_title("Virtual Network");
    if virtual_networks.is_empty() {
        let empty_list = gtk::StringList::new(&["(none)"]);
        virt_net_row.set_model(Some(&empty_list));
        virt_net_row.set_sensitive(false);
    } else {
        let virt_net_list = gtk::StringList::new(&virt_net_labels);
        virt_net_row.set_model(Some(&virt_net_list));
        // Select "default" if present, else index 0
        let default_idx = virtual_networks.iter().position(|n| n == "default").unwrap_or(0);
        virt_net_row.set_selected(default_idx as u32);
    }
    network_group.add(&virt_net_row);

    // Device name entry row (for Bridge / Macvtap / vDPA)
    let dev_entry_row = adw::EntryRow::new();
    dev_entry_row.set_title("Device Name");
    dev_entry_row.set_visible(false);
    network_group.add(&dev_entry_row);

    // Model
    let model_labels: Vec<&str> = NetworkModel::ALL.iter().map(|m| m.label()).collect();
    let model_list = gtk::StringList::new(&model_labels);
    let model_row = adw::ComboRow::new();
    model_row.set_title("Model");
    model_row.set_model(Some(&model_list));
    model_row.set_selected(0); // virtio
    network_group.add(&model_row);

    content.append(&network_group);

    // Wire up source type selection to show/hide rows
    let virt_net_row_clone = virt_net_row.clone();
    let dev_entry_row_clone = dev_entry_row.clone();
    src_type_row.connect_notify_local(Some("selected"), move |row, _| {
        let idx = row.selected() as usize;
        let src = NetworkSourceType::ALL.get(idx).copied().unwrap_or(NetworkSourceType::VirtualNetwork);
        match src {
            NetworkSourceType::VirtualNetwork => {
                virt_net_row_clone.set_visible(true);
                dev_entry_row_clone.set_visible(false);
                dev_entry_row_clone.set_title("Device Name");
            }
            NetworkSourceType::Bridge => {
                virt_net_row_clone.set_visible(false);
                dev_entry_row_clone.set_title("Bridge Device");
                dev_entry_row_clone.set_visible(true);
            }
            NetworkSourceType::Macvtap => {
                virt_net_row_clone.set_visible(false);
                dev_entry_row_clone.set_title("Macvtap Device");
                dev_entry_row_clone.set_visible(true);
            }
            NetworkSourceType::Vdpa => {
                virt_net_row_clone.set_visible(false);
                dev_entry_row_clone.set_title("vDPA Device");
                dev_entry_row_clone.set_visible(true);
            }
        }
    });

    // Browse button handler — opens the libvirt storage volume picker
    let iso_path_clone = iso_path.clone();
    let iso_row_clone = iso_row.clone();
    let clear_btn_clone = clear_btn.clone();
    let parent_clone = parent.clone();
    let pool_volumes_clone = pool_volumes.clone();
    browse_btn.connect_clicked(move |_| {
        let iso_path = iso_path_clone.clone();
        let iso_row = iso_row_clone.clone();
        let clear_btn = clear_btn_clone.clone();
        crate::ui::storage_volume_picker_dialog::show_storage_volume_picker(
            &parent_clone,
            &pool_volumes_clone,
            move |path| {
                let display = std::path::Path::new(&path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.clone());
                iso_row.set_subtitle(&display);
                *iso_path.borrow_mut() = Some(path);
                clear_btn.set_visible(true);
            },
        );
    });

    // Clear button handler
    let iso_path_clone = iso_path.clone();
    let iso_row_clone = iso_row.clone();
    clear_btn.connect_clicked(move |btn| {
        *iso_path_clone.borrow_mut() = None;
        iso_row_clone.set_subtitle("No ISO selected");
        btn.set_visible(false);
    });

    // Create button
    let create_btn = gtk::Button::with_label("Create");
    create_btn.add_css_class("suggested-action");
    create_btn.add_css_class("pill");
    create_btn.set_halign(gtk::Align::Center);
    create_btn.set_margin_top(12);
    content.append(&create_btn);

    clamp.set_child(Some(&content));
    toolbar_view.set_content(Some(&clamp));
    dialog.set_child(Some(&toolbar_view));

    let dialog_ref = dialog.clone();
    create_btn.connect_clicked(move |_| {
        let fw_idx = firmware_row.selected() as usize;
        let firmware = FirmwareType::ALL.get(fw_idx).copied().unwrap_or(FirmwareType::Bios);
        let fmt_idx = format_row.selected() as usize;
        let disk_format = DiskFormat::ALL.get(fmt_idx).copied().unwrap_or(DiskFormat::Qcow2);

        let src_idx = src_type_row.selected() as usize;
        let source_type = NetworkSourceType::ALL.get(src_idx).copied().unwrap_or(NetworkSourceType::VirtualNetwork);
        let source_value = match source_type {
            NetworkSourceType::VirtualNetwork => {
                let idx = virt_net_row.selected() as usize;
                virtual_networks.get(idx).cloned().unwrap_or_else(|| "default".to_string())
            }
            _ => dev_entry_row.text().to_string(),
        };
        let model_idx = model_row.selected() as usize;
        let model = NetworkModel::ALL.get(model_idx).copied().unwrap_or(NetworkModel::Virtio);

        let params = NewVmParams {
            name: name_row.text().to_string(),
            vcpus: cpu_row.value() as u32,
            memory_mib: memory_row.value() as u64,
            disk_size_gib: disk_row.value() as u64,
            disk_format,
            iso_path: iso_path.borrow().clone(),
            firmware,
            network: NewVmNetworkConfig { source_type, source_value, model },
        };

        if params.name.is_empty() {
            return;
        }

        on_create(params);
        dialog_ref.close();
    });

    dialog.present();
}
