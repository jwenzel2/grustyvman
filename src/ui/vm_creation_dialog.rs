use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::backend::domain_xml::NewVmParams;
use crate::backend::osinfo::{load_os_list, OsEntry};
use crate::backend::types::{DiskFormat, FirmwareType, NetworkModel, NetworkSourceType, NewVmNetworkConfig, TpmModel, VolumeInfo};

pub fn show_creation_dialog(
    parent: &adw::ApplicationWindow,
    pool_volumes: Vec<(String, Vec<VolumeInfo>)>,
    virtual_networks: Vec<String>,
    on_create: impl Fn(NewVmParams) + 'static,
) {
    let dialog = gtk::Window::new();
    dialog.set_title(Some("New Virtual Machine"));
    dialog.set_default_size(480, 560);
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

    // OS search row
    let os_row = adw::EntryRow::new();
    os_row.set_title("Operating System");
    os_row.set_show_apply_button(false);
    general_group.add(&os_row);

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

    // TPM group
    let tpm_group = adw::PreferencesGroup::new();
    tpm_group.set_title("Security");

    let tpm_enable_row = adw::SwitchRow::new();
    tpm_enable_row.set_title("Enable TPM");
    tpm_enable_row.set_active(false);
    tpm_group.add(&tpm_enable_row);

    // Only show real models (not None)
    let tpm_models: Vec<TpmModel> = TpmModel::ALL.iter().copied().filter(|m| *m != TpmModel::None).collect();
    let tpm_model_labels: Vec<&str> = tpm_models.iter().map(|m| m.label()).collect();
    let tpm_model_list = gtk::StringList::new(&tpm_model_labels);
    let tpm_model_row = adw::ComboRow::new();
    tpm_model_row.set_title("TPM Model");
    tpm_model_row.set_model(Some(&tpm_model_list));
    tpm_model_row.set_selected(0); // CRB
    tpm_model_row.set_sensitive(false);
    tpm_group.add(&tpm_model_row);

    content.append(&tpm_group);

    // Wire enable switch to model row sensitivity
    let tpm_model_row_clone = tpm_model_row.clone();
    tpm_enable_row.connect_notify_local(Some("active"), move |row, _| {
        tpm_model_row_clone.set_sensitive(row.is_active());
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

    // --- OS search popover (set up after dialog tree is built) ---
    // Load OS list from osinfo-db (matches virt-manager's OS list)
    let os_entries: Rc<Vec<OsEntry>> = Rc::new(load_os_list());

    // Tracks which OS the user has confirmed by selecting from the list
    let selected_os: Rc<RefCell<Option<OsEntry>>> = Rc::new(RefCell::new(None));

    // Tracks results currently shown in the popover (index matches ListBox row index)
    let current_results: Rc<RefCell<Vec<OsEntry>>> = Rc::new(RefCell::new(Vec::new()));

    // Guard to avoid re-triggering 'changed' when we programmatically set text
    let updating_text = Rc::new(RefCell::new(false));

    // Build the popover
    let os_list_box = gtk::ListBox::new();
    os_list_box.set_selection_mode(gtk::SelectionMode::Browse);

    let os_scroll = gtk::ScrolledWindow::new();
    os_scroll.set_size_request(440, 240);
    os_scroll.set_child(Some(&os_list_box));

    let os_popover = gtk::Popover::new();
    os_popover.set_has_arrow(false);
    os_popover.set_autohide(true);
    os_popover.set_position(gtk::PositionType::Bottom);
    os_popover.set_child(Some(&os_scroll));
    os_popover.set_parent(&os_row);

    // Wire up: typing in os_row filters the list and shows/hides the popover
    let os_entries_c1 = os_entries.clone();
    let current_results_c1 = current_results.clone();
    let selected_os_c1 = selected_os.clone();
    let updating_c1 = updating_text.clone();
    let os_list_box_c1 = os_list_box.clone();
    let os_popover_c1 = os_popover.clone();

    os_row.connect_changed(move |entry| {
        if *updating_c1.borrow() {
            return;
        }

        // Any manual edit clears the confirmed selection
        *selected_os_c1.borrow_mut() = None;

        let query = entry.text().to_lowercase();

        // Clear existing rows
        while let Some(child) = os_list_box_c1.first_child() {
            os_list_box_c1.remove(&child);
        }

        if query.len() < 2 {
            os_popover_c1.popdown();
            *current_results_c1.borrow_mut() = Vec::new();
            return;
        }

        // Filter by name or short_id, cap at 80 results
        let mut results: Vec<OsEntry> = Vec::new();
        for entry in os_entries_c1.iter() {
            if results.len() >= 80 {
                break;
            }
            if entry.name.to_lowercase().contains(&query)
                || entry.short_id.to_lowercase().contains(&query)
            {
                results.push(entry.clone());
            }
        }

        if results.is_empty() {
            os_popover_c1.popdown();
            *current_results_c1.borrow_mut() = Vec::new();
            return;
        }

        // Populate list box
        for result in &results {
            let row = gtk::ListBoxRow::new();
            let vbox = gtk::Box::new(gtk::Orientation::Vertical, 1);
            vbox.set_margin_start(8);
            vbox.set_margin_end(8);
            vbox.set_margin_top(6);
            vbox.set_margin_bottom(6);

            let name_label = gtk::Label::new(Some(&result.name));
            name_label.set_halign(gtk::Align::Start);
            name_label.set_ellipsize(gtk::pango::EllipsizeMode::End);

            let id_label = gtk::Label::new(Some(&result.short_id));
            id_label.set_halign(gtk::Align::Start);
            id_label.add_css_class("caption");
            id_label.add_css_class("dim-label");

            vbox.append(&name_label);
            vbox.append(&id_label);
            row.set_child(Some(&vbox));
            os_list_box_c1.append(&row);
        }

        *current_results_c1.borrow_mut() = results;
        os_popover_c1.popup();
    });

    // Wire up: activating a list row fills in the entry and stores the selection
    let current_results_c2 = current_results.clone();
    let selected_os_c2 = selected_os.clone();
    let updating_c2 = updating_text.clone();
    let os_row_c2 = os_row.clone();
    let os_popover_c2 = os_popover.clone();

    os_list_box.connect_row_activated(move |_, row| {
        let idx = row.index() as usize;
        let results = current_results_c2.borrow();
        if let Some(entry) = results.get(idx) {
            *selected_os_c2.borrow_mut() = Some(entry.clone());
            *updating_c2.borrow_mut() = true;
            os_row_c2.set_text(&entry.name);
            *updating_c2.borrow_mut() = false;
            os_popover_c2.popdown();
        }
    });
    // --- end OS search popover ---

    let selected_os_c3 = selected_os.clone();
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

        let tpm_model = if tpm_enable_row.is_active() {
            let idx = tpm_model_row.selected() as usize;
            tpm_models.get(idx).copied()
        } else {
            None
        };

        let os_variant_id = selected_os_c3.borrow().as_ref().map(|e| e.id.clone());

        let params = NewVmParams {
            name: name_row.text().to_string(),
            vcpus: cpu_row.value() as u32,
            memory_mib: memory_row.value() as u64,
            disk_size_gib: disk_row.value() as u64,
            disk_format,
            iso_path: iso_path.borrow().clone(),
            firmware,
            network: NewVmNetworkConfig { source_type, source_value, model },
            tpm_model,
            os_variant_id,
        };

        if params.name.is_empty() {
            return;
        }

        on_create(params);
        dialog_ref.close();
    });

    dialog.present();
}
