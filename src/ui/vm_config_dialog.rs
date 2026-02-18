use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::backend::types::{
    BootDevice, ConfigAction, ConfigChanges, CpuMode, CpuTune, DomainDetails, FilesystemInfo,
    FirmwareType, GraphicsType, SoundModel, TpmModel, VcpuPin, VideoModel, CPU_MODELS,
};

pub fn show_config_dialog(
    parent: &adw::ApplicationWindow,
    details: &DomainDetails,
    autostart: bool,
    is_running: bool,
    networks: Vec<String>,
    host_cpu_count: u32,
    on_action: impl Fn(ConfigAction) + Clone + 'static,
) {
    let window = adw::PreferencesWindow::new();
    window.set_title(Some("VM Settings"));
    window.set_default_size(600, 700);
    window.set_modal(true);
    window.set_transient_for(Some(parent));
    window.set_search_enabled(false);

    // --- Overview page ---
    let overview_page = adw::PreferencesPage::new();
    overview_page.set_title("Overview");
    overview_page.set_icon_name(Some("preferences-system-symbolic"));

    if is_running {
        let banner_group = adw::PreferencesGroup::new();
        let banner = adw::Banner::new("VM is running. Changes take effect after restart.");
        banner.set_revealed(true);
        banner_group.add(&banner);
        overview_page.add(&banner_group);
    }

    // Resources group
    let resources_group = adw::PreferencesGroup::new();
    resources_group.set_title("Resources");

    let cpu_row = adw::SpinRow::with_range(1.0, 32.0, 1.0);
    cpu_row.set_title("vCPUs");
    cpu_row.set_value(details.vcpus as f64);
    resources_group.add(&cpu_row);

    let memory_row = adw::SpinRow::with_range(256.0, 65536.0, 256.0);
    memory_row.set_title("Memory (MiB)");
    memory_row.set_value((details.memory_kib / 1024) as f64);
    resources_group.add(&memory_row);

    overview_page.add(&resources_group);

    // CPU group
    let cpu_group = adw::PreferencesGroup::new();
    cpu_group.set_title("CPU");

    let cpu_mode_list = gtk::StringList::new(&CpuMode::ALL.iter().map(|m| m.label()).collect::<Vec<_>>());
    let cpu_mode_row = adw::ComboRow::new();
    cpu_mode_row.set_title("CPU Mode");
    cpu_mode_row.set_model(Some(&cpu_mode_list));
    let current_mode_idx = CpuMode::ALL.iter().position(|m| *m == details.cpu_mode).unwrap_or(0);
    cpu_mode_row.set_selected(current_mode_idx as u32);
    cpu_group.add(&cpu_mode_row);

    let model_labels: Vec<&str> = CPU_MODELS.to_vec();
    let cpu_model_list = gtk::StringList::new(&model_labels);
    let cpu_model_row = adw::ComboRow::new();
    cpu_model_row.set_title("CPU Model");
    cpu_model_row.set_model(Some(&cpu_model_list));

    if let Some(ref model) = details.cpu_model {
        let idx = CPU_MODELS.iter().position(|m| *m == model.as_str()).unwrap_or(0);
        cpu_model_row.set_selected(idx as u32);
    }

    cpu_model_row.set_visible(details.cpu_mode == CpuMode::Custom);
    cpu_group.add(&cpu_model_row);

    let cpu_model_row_ref = cpu_model_row.clone();
    cpu_mode_row.connect_selected_notify(move |row| {
        let idx = row.selected() as usize;
        let mode = CpuMode::ALL.get(idx).copied().unwrap_or(CpuMode::HostPassthrough);
        cpu_model_row_ref.set_visible(mode == CpuMode::Custom);
    });

    overview_page.add(&cpu_group);

    // Firmware group
    let firmware_group = adw::PreferencesGroup::new();
    firmware_group.set_title("Firmware");

    let firmware_labels: Vec<&str> = FirmwareType::ALL.iter().map(|f| f.label()).collect();
    let firmware_list = gtk::StringList::new(&firmware_labels);
    let firmware_row = adw::ComboRow::new();
    firmware_row.set_title("Firmware Type");
    firmware_row.set_model(Some(&firmware_list));
    let current_fw_idx = FirmwareType::ALL.iter().position(|f| *f == details.firmware).unwrap_or(0);
    firmware_row.set_selected(current_fw_idx as u32);
    firmware_group.add(&firmware_row);

    overview_page.add(&firmware_group);

    // General group (autostart)
    let general_group = adw::PreferencesGroup::new();
    general_group.set_title("General");

    let autostart_row = adw::SwitchRow::new();
    autostart_row.set_title("Autostart");
    autostart_row.set_subtitle("Start this VM when the host boots");
    autostart_row.set_active(autostart);
    general_group.add(&autostart_row);

    overview_page.add(&general_group);

    // CPU Pinning group
    let pinning_group = adw::PreferencesGroup::new();
    pinning_group.set_title("CPU Pinning");
    if host_cpu_count > 0 {
        pinning_group.set_description(Some(&format!(
            "Host has {} logical CPUs (0-{})",
            host_cpu_count,
            host_cpu_count - 1
        )));
    }

    let vcpu_count = details.vcpus;
    let pin_entries: Rc<RefCell<Vec<adw::EntryRow>>> = Rc::new(RefCell::new(Vec::new()));

    for i in 0..vcpu_count {
        let entry = adw::EntryRow::new();
        entry.set_title(&format!("vCPU {i}"));
        // Pre-fill with existing pin value
        if let Some(pin) = details.cpu_tune.vcpu_pins.iter().find(|p| p.vcpu == i) {
            entry.set_text(&pin.cpuset);
        }
        pinning_group.add(&entry);
        pin_entries.borrow_mut().push(entry);
    }

    let emulator_pin_entry = adw::EntryRow::new();
    emulator_pin_entry.set_title("Emulator Thread");
    if let Some(ref cpuset) = details.cpu_tune.emulatorpin {
        emulator_pin_entry.set_text(cpuset);
    }
    pinning_group.add(&emulator_pin_entry);

    let pin_apply_btn = gtk::Button::with_label("Apply CPU Pinning");
    pin_apply_btn.add_css_class("suggested-action");
    pin_apply_btn.add_css_class("pill");
    pin_apply_btn.set_halign(gtk::Align::Center);
    pin_apply_btn.set_margin_top(8);

    let on_action_pin = on_action.clone();
    let window_ref_pin = window.clone();
    let pin_entries_ref = pin_entries.clone();
    let emulator_entry_ref = emulator_pin_entry.clone();
    pin_apply_btn.connect_clicked(move |_| {
        let entries = pin_entries_ref.borrow();
        let mut vcpu_pins = Vec::new();
        for (i, entry) in entries.iter().enumerate() {
            let text = entry.text().trim().to_string();
            if !text.is_empty() {
                vcpu_pins.push(VcpuPin {
                    vcpu: i as u32,
                    cpuset: text,
                });
            }
        }
        let emulatorpin = {
            let text = emulator_entry_ref.text().trim().to_string();
            if text.is_empty() { None } else { Some(text) }
        };
        on_action_pin(ConfigAction::ApplyCpuTune(CpuTune {
            vcpu_pins,
            emulatorpin,
        }));
        window_ref_pin.close();
    });
    pinning_group.add(&pin_apply_btn);

    overview_page.add(&pinning_group);

    // Apply button group
    let apply_group = adw::PreferencesGroup::new();
    let apply_btn = gtk::Button::with_label("Apply");
    apply_btn.add_css_class("suggested-action");
    apply_btn.add_css_class("pill");
    apply_btn.set_halign(gtk::Align::Center);
    apply_btn.set_margin_top(12);
    apply_group.add(&apply_btn);
    overview_page.add(&apply_group);

    let on_action_apply = on_action.clone();
    let window_ref = window.clone();
    let boot_order = details.boot_order.clone();
    apply_btn.connect_clicked(move |_| {
        let mode_idx = cpu_mode_row.selected() as usize;
        let mode = CpuMode::ALL.get(mode_idx).copied().unwrap_or(CpuMode::HostPassthrough);

        let model = if mode == CpuMode::Custom {
            let model_idx = cpu_model_row.selected() as usize;
            CPU_MODELS.get(model_idx).map(|s| s.to_string())
        } else {
            None
        };

        let fw_idx = firmware_row.selected() as usize;
        let firmware = FirmwareType::ALL.get(fw_idx).copied().unwrap_or(FirmwareType::Bios);

        let changes = ConfigChanges {
            vcpus: cpu_row.value() as u32,
            memory_mib: memory_row.value() as u64,
            cpu_mode: mode,
            cpu_model: model,
            boot_order: boot_order.clone(),
            autostart: autostart_row.is_active(),
            firmware,
        };
        on_action_apply(ConfigAction::ApplyGeneral(changes));
        window_ref.close();
    });

    window.add(&overview_page);

    // --- Boot Order page ---
    let boot_page = adw::PreferencesPage::new();
    boot_page.set_title("Boot Order");
    boot_page.set_icon_name(Some("media-optical-symbolic"));

    let boot_group = adw::PreferencesGroup::new();
    boot_group.set_title("Boot Device Order");
    boot_group.set_description(Some("Devices are tried in order from top to bottom"));

    let boot_list = Rc::new(RefCell::new(details.boot_order.clone()));

    let boot_listbox = gtk::ListBox::new();
    boot_listbox.add_css_class("boxed-list");
    boot_listbox.set_selection_mode(gtk::SelectionMode::None);

    fn rebuild_boot_list(listbox: &gtk::ListBox, boot_list: &Rc<RefCell<Vec<BootDevice>>>) {
        while let Some(child) = listbox.first_child() {
            listbox.remove(&child);
        }
        let list = boot_list.borrow();
        for (i, dev) in list.iter().enumerate() {
            let row = adw::ActionRow::new();
            row.set_title(&format!("{}. {}", i + 1, dev.label()));
            row.set_activatable(false);

            let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            btn_box.set_valign(gtk::Align::Center);

            let up_btn = gtk::Button::from_icon_name("go-up-symbolic");
            up_btn.set_sensitive(i > 0);
            up_btn.add_css_class("flat");
            let bl = boot_list.clone();
            let lb = listbox.clone();
            let idx = i;
            up_btn.connect_clicked(move |_| {
                let mut l = bl.borrow_mut();
                if idx > 0 {
                    l.swap(idx, idx - 1);
                }
                drop(l);
                rebuild_boot_list(&lb, &bl);
            });

            let down_btn = gtk::Button::from_icon_name("go-down-symbolic");
            down_btn.set_sensitive(i < list.len() - 1);
            down_btn.add_css_class("flat");
            let bl = boot_list.clone();
            let lb = listbox.clone();
            let idx = i;
            let len = list.len();
            down_btn.connect_clicked(move |_| {
                let mut l = bl.borrow_mut();
                if idx + 1 < len {
                    l.swap(idx, idx + 1);
                }
                drop(l);
                rebuild_boot_list(&lb, &bl);
            });

            let remove_btn = gtk::Button::from_icon_name("list-remove-symbolic");
            remove_btn.add_css_class("flat");
            let bl = boot_list.clone();
            let lb = listbox.clone();
            let idx = i;
            remove_btn.connect_clicked(move |_| {
                bl.borrow_mut().remove(idx);
                rebuild_boot_list(&lb, &bl);
            });

            btn_box.append(&up_btn);
            btn_box.append(&down_btn);
            btn_box.append(&remove_btn);
            row.add_suffix(&btn_box);

            listbox.append(&row);
        }
    }

    rebuild_boot_list(&boot_listbox, &boot_list);
    boot_group.add(&boot_listbox);

    // Add new boot device
    let add_boot_group = adw::PreferencesGroup::new();

    let boot_device_labels: Vec<&str> = BootDevice::ALL.iter().map(|d| d.label()).collect();
    let boot_device_list = gtk::StringList::new(&boot_device_labels);
    let boot_device_combo = adw::ComboRow::new();
    boot_device_combo.set_title("Device Type");
    boot_device_combo.set_model(Some(&boot_device_list));
    add_boot_group.add(&boot_device_combo);

    let add_boot_btn = gtk::Button::with_label("Add Boot Device");
    add_boot_btn.add_css_class("flat");
    add_boot_btn.set_halign(gtk::Align::Center);
    let bl = boot_list.clone();
    let lb = boot_listbox.clone();
    let combo = boot_device_combo.clone();
    add_boot_btn.connect_clicked(move |_| {
        let idx = combo.selected() as usize;
        if let Some(dev) = BootDevice::ALL.get(idx) {
            bl.borrow_mut().push(*dev);
            rebuild_boot_list(&lb, &bl);
        }
    });
    add_boot_group.add(&add_boot_btn);

    boot_page.add(&boot_group);
    boot_page.add(&add_boot_group);

    // Apply boot order button
    let boot_apply_group = adw::PreferencesGroup::new();
    let boot_apply_btn = gtk::Button::with_label("Apply Boot Order");
    boot_apply_btn.add_css_class("suggested-action");
    boot_apply_btn.add_css_class("pill");
    boot_apply_btn.set_halign(gtk::Align::Center);
    boot_apply_btn.set_margin_top(12);

    let on_action_boot = on_action.clone();
    let bl = boot_list.clone();
    let window_ref = window.clone();
    boot_apply_btn.connect_clicked(move |_| {
        let devices = bl.borrow().clone();
        // We send a general config change with just the boot order updated
        // The window handler will use modify_boot_order
        on_action_boot(ConfigAction::ApplyGeneral(ConfigChanges {
            vcpus: 0,  // signals to only apply boot order
            memory_mib: 0,
            cpu_mode: CpuMode::HostPassthrough,
            cpu_model: None,
            boot_order: devices,
            autostart: false,
            firmware: FirmwareType::Bios,
        }));
        window_ref.close();
    });
    boot_apply_group.add(&boot_apply_btn);
    boot_page.add(&boot_apply_group);

    window.add(&boot_page);

    // --- Devices page ---
    let devices_page = adw::PreferencesPage::new();
    devices_page.set_title("Devices");
    devices_page.set_icon_name(Some("drive-harddisk-symbolic"));

    // Disks group
    let disks_group = adw::PreferencesGroup::new();
    disks_group.set_title("Disks");

    let add_disk_btn = gtk::Button::from_icon_name("list-add-symbolic");
    add_disk_btn.set_tooltip_text(Some("Add Disk"));
    add_disk_btn.add_css_class("flat");
    disks_group.set_header_suffix(Some(&add_disk_btn));

    for disk in &details.disks {
        let row = adw::ActionRow::new();
        let type_label = if disk.device_type == "cdrom" { " (CD-ROM)" } else { "" };
        row.set_title(&format!("/dev/{}{}", disk.target_dev, type_label));
        row.set_subtitle(&disk.source_file.clone().unwrap_or_else(|| "No media".to_string()));

        let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        btn_box.set_valign(gtk::Align::Center);

        if disk.device_type == "cdrom" {
            // Eject button
            let eject_btn = gtk::Button::from_icon_name("media-eject-symbolic");
            eject_btn.add_css_class("flat");
            eject_btn.set_tooltip_text(Some("Eject Media"));
            eject_btn.set_sensitive(disk.source_file.is_some());
            let on_action_eject = on_action.clone();
            let target = disk.target_dev.clone();
            let window_ref = window.clone();
            eject_btn.connect_clicked(move |_| {
                on_action_eject(ConfigAction::EjectCdrom(target.clone()));
                window_ref.close();
            });
            btn_box.append(&eject_btn);

            // Change media button
            let change_btn = gtk::Button::from_icon_name("document-open-symbolic");
            change_btn.add_css_class("flat");
            change_btn.set_tooltip_text(Some("Change Media"));
            let on_action_change = on_action.clone();
            let target = disk.target_dev.clone();
            let window_ref = window.clone();
            let parent_ref = parent.clone();
            change_btn.connect_clicked(move |_| {
                let on_action = on_action_change.clone();
                let target = target.clone();
                let wr = window_ref.clone();
                let dialog = gtk::FileDialog::builder()
                    .title("Select ISO Image")
                    .build();
                let filter = gtk::FileFilter::new();
                filter.add_pattern("*.iso");
                filter.add_pattern("*.ISO");
                filter.set_name(Some("ISO Images"));
                let filters = gio::ListStore::new::<gtk::FileFilter>();
                filters.append(&filter);
                dialog.set_filters(Some(&filters));
                dialog.open(Some(&parent_ref), gtk::gio::Cancellable::NONE, move |result| {
                    if let Ok(file) = result {
                        if let Some(path) = file.path() {
                            on_action(ConfigAction::InsertCdrom(
                                target.clone(),
                                path.to_string_lossy().to_string(),
                            ));
                            wr.close();
                        }
                    }
                });
            });
            btn_box.append(&change_btn);
        }

        // Remove button (for all disks)
        let remove_btn = gtk::Button::from_icon_name("user-trash-symbolic");
        remove_btn.add_css_class("flat");
        remove_btn.set_tooltip_text(Some("Remove Disk"));
        let on_action_disk = on_action.clone();
        let target = disk.target_dev.clone();
        let window_ref = window.clone();
        remove_btn.connect_clicked(move |_| {
            on_action_disk(ConfigAction::RemoveDisk(target.clone()));
            window_ref.close();
        });
        btn_box.append(&remove_btn);

        row.add_suffix(&btn_box);
        row.set_activatable(false);
        disks_group.add(&row);
    }

    let on_action_add_disk = on_action.clone();
    let parent_ref = parent.clone();
    let window_ref = window.clone();
    let details_clone = details.clone();
    add_disk_btn.connect_clicked(move |_| {
        crate::ui::add_disk_dialog::show_add_disk_dialog(
            &parent_ref,
            &details_clone,
            {
                let on_action = on_action_add_disk.clone();
                let wr = window_ref.clone();
                move |params| {
                    on_action(ConfigAction::AddDisk(params));
                    wr.close();
                }
            },
        );
    });

    devices_page.add(&disks_group);

    // Network group
    let networks_group = adw::PreferencesGroup::new();
    networks_group.set_title("Network Interfaces");

    let add_net_btn = gtk::Button::from_icon_name("list-add-symbolic");
    add_net_btn.set_tooltip_text(Some("Add Network Interface"));
    add_net_btn.add_css_class("flat");
    networks_group.set_header_suffix(Some(&add_net_btn));

    for net in &details.networks {
        let row = adw::ActionRow::new();
        let title = net.mac_address.clone().unwrap_or_else(|| "Unknown MAC".to_string());
        row.set_title(&title);
        let subtitle = format!(
            "Network: {} | Model: {}",
            net.source_network.as_deref().unwrap_or("N/A"),
            net.model_type.as_deref().unwrap_or("N/A")
        );
        row.set_subtitle(&subtitle);

        let remove_btn = gtk::Button::from_icon_name("user-trash-symbolic");
        remove_btn.add_css_class("flat");
        remove_btn.set_valign(gtk::Align::Center);
        let on_action_net = on_action.clone();
        let mac = net.mac_address.clone().unwrap_or_default();
        let window_ref = window.clone();
        remove_btn.connect_clicked(move |_| {
            on_action_net(ConfigAction::RemoveNetwork(mac.clone()));
            window_ref.close();
        });
        row.add_suffix(&remove_btn);
        row.set_activatable(false);
        networks_group.add(&row);
    }

    let on_action_add_net = on_action.clone();
    let parent_ref = parent.clone();
    let window_ref = window.clone();
    add_net_btn.connect_clicked(move |_| {
        crate::ui::add_network_dialog::show_add_network_dialog(
            &parent_ref,
            &networks,
            {
                let on_action = on_action_add_net.clone();
                let wr = window_ref.clone();
                move |params| {
                    on_action(ConfigAction::AddNetwork(params));
                    wr.close();
                }
            },
        );
    });

    devices_page.add(&networks_group);

    // TPM group
    let tpm_group = adw::PreferencesGroup::new();
    tpm_group.set_title("TPM");

    let tpm_labels: Vec<&str> = TpmModel::ALL.iter().map(|t| t.label()).collect();
    let tpm_list = gtk::StringList::new(&tpm_labels);
    let tpm_row = adw::ComboRow::new();
    tpm_row.set_title("TPM Model");
    tpm_row.set_model(Some(&tpm_list));
    let current_tpm_idx = details
        .tpm
        .as_ref()
        .and_then(|t| TpmModel::ALL.iter().position(|m| *m == t.model))
        .unwrap_or(TpmModel::ALL.len() - 1); // default to None
    tpm_row.set_selected(current_tpm_idx as u32);
    tpm_group.add(&tpm_row);

    let tpm_apply_btn = gtk::Button::with_label("Apply TPM");
    tpm_apply_btn.add_css_class("suggested-action");
    tpm_apply_btn.add_css_class("pill");
    tpm_apply_btn.set_halign(gtk::Align::Center);
    tpm_apply_btn.set_margin_top(8);

    let on_action_tpm = on_action.clone();
    let window_ref_tpm = window.clone();
    tpm_apply_btn.connect_clicked(move |_| {
        let idx = tpm_row.selected() as usize;
        let model = TpmModel::ALL.get(idx).copied().unwrap_or(TpmModel::None);
        on_action_tpm(ConfigAction::ModifyTpm(model));
        window_ref_tpm.close();
    });
    tpm_group.add(&tpm_apply_btn);

    devices_page.add(&tpm_group);

    // Shared Folders group
    let fs_group = adw::PreferencesGroup::new();
    fs_group.set_title("Shared Folders");

    let add_fs_btn = gtk::Button::from_icon_name("list-add-symbolic");
    add_fs_btn.set_tooltip_text(Some("Add Shared Folder"));
    add_fs_btn.add_css_class("flat");
    fs_group.set_header_suffix(Some(&add_fs_btn));

    for fs in &details.filesystems {
        let row = adw::ActionRow::new();
        row.set_title(&fs.target_dir);
        row.set_subtitle(&format!("{} ({})", fs.source_dir, fs.driver));

        let remove_btn = gtk::Button::from_icon_name("user-trash-symbolic");
        remove_btn.add_css_class("flat");
        remove_btn.set_valign(gtk::Align::Center);
        let on_action_fs_rm = on_action.clone();
        let target = fs.target_dir.clone();
        let window_ref = window.clone();
        remove_btn.connect_clicked(move |_| {
            on_action_fs_rm(ConfigAction::RemoveFilesystem(target.clone()));
            window_ref.close();
        });
        row.add_suffix(&remove_btn);
        row.set_activatable(false);
        fs_group.add(&row);
    }

    devices_page.add(&fs_group);

    // Add Shared Folder form
    let fs_add_group = adw::PreferencesGroup::new();

    let fs_host_path_entry = adw::EntryRow::new();
    fs_host_path_entry.set_title("Host Path");
    fs_add_group.add(&fs_host_path_entry);

    let fs_mount_tag_entry = adw::EntryRow::new();
    fs_mount_tag_entry.set_title("Mount Tag");
    fs_add_group.add(&fs_mount_tag_entry);

    let fs_driver_labels = ["virtio-9p", "virtiofs"];
    let fs_driver_list = gtk::StringList::new(&fs_driver_labels);
    let fs_driver_row = adw::ComboRow::new();
    fs_driver_row.set_title("Driver");
    fs_driver_row.set_model(Some(&fs_driver_list));
    fs_add_group.add(&fs_driver_row);

    let fs_add_apply_btn = gtk::Button::with_label("Add Shared Folder");
    fs_add_apply_btn.add_css_class("suggested-action");
    fs_add_apply_btn.add_css_class("pill");
    fs_add_apply_btn.set_halign(gtk::Align::Center);
    fs_add_apply_btn.set_margin_top(8);

    let on_action_fs_add = on_action.clone();
    let window_ref_fs = window.clone();
    fs_add_apply_btn.connect_clicked(move |_| {
        let host_path = fs_host_path_entry.text().trim().to_string();
        let mount_tag = fs_mount_tag_entry.text().trim().to_string();
        if host_path.is_empty() || mount_tag.is_empty() {
            return;
        }
        let driver_idx = fs_driver_row.selected() as usize;
        let driver = if driver_idx == 1 { "virtiofs" } else { "9p" }.to_string();
        let accessmode = if driver == "9p" {
            Some("mapped".to_string())
        } else {
            None
        };
        on_action_fs_add(ConfigAction::AddFilesystem(FilesystemInfo {
            driver,
            source_dir: host_path,
            target_dir: mount_tag,
            accessmode,
        }));
        window_ref_fs.close();
    });
    fs_add_group.add(&fs_add_apply_btn);

    devices_page.add(&fs_add_group);

    // Host Devices (PCI/USB passthrough) group
    let hostdev_group = adw::PreferencesGroup::new();
    hostdev_group.set_title("Host Devices (PCI/USB Passthrough)");

    let add_hostdev_btn = gtk::Button::from_icon_name("list-add-symbolic");
    add_hostdev_btn.set_tooltip_text(Some("Add Host Device"));
    add_hostdev_btn.add_css_class("flat");
    hostdev_group.set_header_suffix(Some(&add_hostdev_btn));

    for hdev in &details.hostdevs {
        let row = adw::ActionRow::new();
        row.set_title(&hdev.display_name);
        row.set_subtitle(&hdev.display_subtitle());

        let remove_btn = gtk::Button::from_icon_name("user-trash-symbolic");
        remove_btn.add_css_class("flat");
        remove_btn.set_valign(gtk::Align::Center);
        let on_action_hdev = on_action.clone();
        let hdev_clone = hdev.clone();
        let window_ref = window.clone();
        remove_btn.connect_clicked(move |_| {
            on_action_hdev(ConfigAction::RemoveHostdev(hdev_clone.clone()));
            window_ref.close();
        });
        row.add_suffix(&remove_btn);
        row.set_activatable(false);
        hostdev_group.add(&row);
    }

    let on_action_hostdev = on_action.clone();
    let parent_ref_hdev = parent.clone();
    let window_ref_hdev = window.clone();
    add_hostdev_btn.connect_clicked(move |_| {
        crate::ui::add_hostdev_dialog::show_add_hostdev_dialog(
            &parent_ref_hdev,
            {
                let on_action = on_action_hostdev.clone();
                let wr = window_ref_hdev.clone();
                move |info| {
                    on_action(ConfigAction::AddHostdev(info));
                    wr.close();
                }
            },
        );
    });

    devices_page.add(&hostdev_group);

    // --- Serial / Console Ports group ---
    let serial_group = adw::PreferencesGroup::new();
    serial_group.set_title("Serial &amp; Console Ports");

    let add_serial_btn = gtk::Button::from_icon_name("list-add-symbolic");
    add_serial_btn.set_tooltip_text(Some("Add Serial/Console Port"));
    add_serial_btn.add_css_class("flat");
    serial_group.set_header_suffix(Some(&add_serial_btn));

    for s in &details.serials {
        let row = adw::ActionRow::new();
        row.set_title(&s.display_name());
        row.set_subtitle(&s.display_subtitle());

        let remove_btn = gtk::Button::from_icon_name("user-trash-symbolic");
        remove_btn.add_css_class("flat");
        remove_btn.set_valign(gtk::Align::Center);
        let on_action_sr = on_action.clone();
        let s_clone = s.clone();
        let window_ref = window.clone();
        remove_btn.connect_clicked(move |_| {
            on_action_sr(ConfigAction::RemoveSerial(s_clone.clone()));
            window_ref.close();
        });
        row.add_suffix(&remove_btn);
        row.set_activatable(false);
        serial_group.add(&row);
    }

    // Add serial form inline
    let serial_add_group = adw::PreferencesGroup::new();

    let serial_type_labels = ["VirtIO Console", "ISA Serial"];
    let serial_type_list = gtk::StringList::new(&serial_type_labels);
    let serial_type_row = adw::ComboRow::new();
    serial_type_row.set_title("Port Type");
    serial_type_row.set_model(Some(&serial_type_list));
    serial_add_group.add(&serial_type_row);

    let serial_apply_btn = gtk::Button::with_label("Add Port");
    serial_apply_btn.add_css_class("suggested-action");
    serial_apply_btn.add_css_class("pill");
    serial_apply_btn.set_halign(gtk::Align::Center);
    serial_apply_btn.set_margin_top(8);
    let on_action_serial = on_action.clone();
    let window_ref_serial = window.clone();
    let next_port = details.serials.len() as u32;
    serial_apply_btn.connect_clicked(move |_| {
        let idx = serial_type_row.selected();
        let (is_console, target_type) = if idx == 0 {
            (true, "virtio".to_string())
        } else {
            (false, "isa-serial".to_string())
        };
        use crate::backend::types::SerialInfo;
        on_action_serial(ConfigAction::AddSerial(SerialInfo {
            is_console,
            target_type,
            port: next_port,
        }));
        window_ref_serial.close();
    });
    serial_add_group.add(&serial_apply_btn);

    // Only show add form when + button clicked
    serial_add_group.set_visible(false);
    let serial_add_group_ref = serial_add_group.clone();
    add_serial_btn.connect_clicked(move |_| {
        serial_add_group_ref.set_visible(!serial_add_group_ref.is_visible());
    });

    devices_page.add(&serial_group);
    devices_page.add(&serial_add_group);

    // --- RNG group ---
    let rng_group = adw::PreferencesGroup::new();
    rng_group.set_title("Random Number Generator");

    use crate::backend::types::RngBackend;
    let rng_labels: Vec<&str> = {
        let mut v: Vec<&str> = RngBackend::ALL.iter().map(|r| r.label()).collect();
        v.push("None (disabled)");
        v
    };
    let rng_str_labels: Vec<String> = rng_labels.iter().map(|s| s.to_string()).collect();
    let rng_label_refs: Vec<&str> = rng_str_labels.iter().map(|s| s.as_str()).collect();
    let rng_list = gtk::StringList::new(&rng_label_refs);
    let rng_row = adw::ComboRow::new();
    rng_row.set_title("Backend");
    rng_row.set_model(Some(&rng_list));
    let current_rng_idx = match details.rng {
        Some(RngBackend::Random) => 0,
        Some(RngBackend::Urandom) => 1,
        None => 2,
    };
    rng_row.set_selected(current_rng_idx);
    rng_group.add(&rng_row);

    let rng_apply_btn = gtk::Button::with_label("Apply RNG");
    rng_apply_btn.add_css_class("suggested-action");
    rng_apply_btn.add_css_class("pill");
    rng_apply_btn.set_halign(gtk::Align::Center);
    rng_apply_btn.set_margin_top(8);
    let on_action_rng = on_action.clone();
    let window_ref_rng = window.clone();
    rng_apply_btn.connect_clicked(move |_| {
        let idx = rng_row.selected() as usize;
        let backend = RngBackend::ALL.get(idx).copied();
        on_action_rng(ConfigAction::ModifyRng(backend));
        window_ref_rng.close();
    });
    rng_group.add(&rng_apply_btn);

    devices_page.add(&rng_group);

    // --- Watchdog group ---
    let wd_group = adw::PreferencesGroup::new();
    wd_group.set_title("Watchdog");

    use crate::backend::types::{WatchdogAction, WatchdogModel};
    let wd_model_labels: Vec<&str> = WatchdogModel::ALL.iter().map(|m| m.label()).collect();
    let wd_model_list = gtk::StringList::new(&wd_model_labels);
    let wd_model_row = adw::ComboRow::new();
    wd_model_row.set_title("Model");
    wd_model_row.set_model(Some(&wd_model_list));
    let current_wd_model_idx = details
        .watchdog
        .as_ref()
        .and_then(|w| WatchdogModel::ALL.iter().position(|m| *m == w.model))
        .unwrap_or(WatchdogModel::ALL.len() - 1);
    wd_model_row.set_selected(current_wd_model_idx as u32);
    wd_group.add(&wd_model_row);

    let wd_action_labels: Vec<&str> = WatchdogAction::ALL.iter().map(|a| a.label()).collect();
    let wd_action_list = gtk::StringList::new(&wd_action_labels);
    let wd_action_row = adw::ComboRow::new();
    wd_action_row.set_title("Action");
    wd_action_row.set_model(Some(&wd_action_list));
    let current_wd_action_idx = details
        .watchdog
        .as_ref()
        .and_then(|w| WatchdogAction::ALL.iter().position(|a| *a == w.action))
        .unwrap_or(0);
    wd_action_row.set_selected(current_wd_action_idx as u32);
    wd_group.add(&wd_action_row);

    let wd_apply_btn = gtk::Button::with_label("Apply Watchdog");
    wd_apply_btn.add_css_class("suggested-action");
    wd_apply_btn.add_css_class("pill");
    wd_apply_btn.set_halign(gtk::Align::Center);
    wd_apply_btn.set_margin_top(8);
    let on_action_wd = on_action.clone();
    let window_ref_wd = window.clone();
    wd_apply_btn.connect_clicked(move |_| {
        let model = WatchdogModel::ALL
            .get(wd_model_row.selected() as usize)
            .copied()
            .unwrap_or(WatchdogModel::None);
        let action = WatchdogAction::ALL
            .get(wd_action_row.selected() as usize)
            .copied()
            .unwrap_or(WatchdogAction::Reset);
        on_action_wd(ConfigAction::ModifyWatchdog(model, action));
        window_ref_wd.close();
    });
    wd_group.add(&wd_apply_btn);

    devices_page.add(&wd_group);

    window.add(&devices_page);

    // --- Display page ---
    let display_page = adw::PreferencesPage::new();
    display_page.set_title("Display");
    display_page.set_icon_name(Some("video-display-symbolic"));

    // Graphics group
    let graphics_group = adw::PreferencesGroup::new();
    graphics_group.set_title("Graphics");

    let graphics_labels: Vec<&str> = GraphicsType::ALL.iter().map(|g| g.label()).collect();
    let graphics_list = gtk::StringList::new(&graphics_labels);
    let graphics_row = adw::ComboRow::new();
    graphics_row.set_title("Graphics Type");
    graphics_row.set_model(Some(&graphics_list));
    let current_gfx_idx = details
        .graphics
        .as_ref()
        .and_then(|g| GraphicsType::ALL.iter().position(|t| *t == g.graphics_type))
        .unwrap_or(GraphicsType::ALL.len() - 1); // default to None
    graphics_row.set_selected(current_gfx_idx as u32);
    graphics_group.add(&graphics_row);

    display_page.add(&graphics_group);

    // Video group
    let video_group = adw::PreferencesGroup::new();
    video_group.set_title("Video");

    let video_labels: Vec<&str> = VideoModel::ALL.iter().map(|v| v.label()).collect();
    let video_list = gtk::StringList::new(&video_labels);
    let video_row = adw::ComboRow::new();
    video_row.set_title("Video Model");
    video_row.set_model(Some(&video_list));
    let current_vid_idx = details
        .video
        .as_ref()
        .and_then(|v| VideoModel::ALL.iter().position(|m| *m == v.model))
        .unwrap_or(VideoModel::ALL.len() - 1);
    video_row.set_selected(current_vid_idx as u32);
    video_group.add(&video_row);

    display_page.add(&video_group);

    // Sound group
    let sound_group = adw::PreferencesGroup::new();
    sound_group.set_title("Sound");

    let sound_labels: Vec<&str> = SoundModel::ALL.iter().map(|s| s.label()).collect();
    let sound_list = gtk::StringList::new(&sound_labels);
    let sound_row = adw::ComboRow::new();
    sound_row.set_title("Sound Model");
    sound_row.set_model(Some(&sound_list));
    let current_snd_idx = details
        .sound
        .as_ref()
        .and_then(|s| SoundModel::ALL.iter().position(|m| *m == s.model))
        .unwrap_or(SoundModel::ALL.len() - 1);
    sound_row.set_selected(current_snd_idx as u32);
    sound_group.add(&sound_row);

    display_page.add(&sound_group);

    // Apply button
    let display_apply_group = adw::PreferencesGroup::new();
    let display_apply_btn = gtk::Button::with_label("Apply Display Settings");
    display_apply_btn.add_css_class("suggested-action");
    display_apply_btn.add_css_class("pill");
    display_apply_btn.set_halign(gtk::Align::Center);
    display_apply_btn.set_margin_top(12);

    let on_action_display = on_action.clone();
    let window_ref = window.clone();
    display_apply_btn.connect_clicked(move |_| {
        let gfx_idx = graphics_row.selected() as usize;
        let gfx = GraphicsType::ALL.get(gfx_idx).copied().unwrap_or(GraphicsType::None);

        let vid_idx = video_row.selected() as usize;
        let vid = VideoModel::ALL.get(vid_idx).copied().unwrap_or(VideoModel::None);

        let snd_idx = sound_row.selected() as usize;
        let snd = SoundModel::ALL.get(snd_idx).copied().unwrap_or(SoundModel::None);

        on_action_display(ConfigAction::ModifyGraphics(gfx));
        on_action_display(ConfigAction::ModifyVideo(vid));
        on_action_display(ConfigAction::ModifySound(snd));
        window_ref.close();
    });
    display_apply_group.add(&display_apply_btn);
    display_page.add(&display_apply_group);

    window.add(&display_page);

    window.present();
}
