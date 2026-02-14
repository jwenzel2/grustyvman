use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::backend::types::{
    BootDevice, ConfigAction, ConfigChanges, CpuMode, DomainDetails, CPU_MODELS,
};

pub fn show_config_dialog(
    parent: &adw::ApplicationWindow,
    details: &DomainDetails,
    autostart: bool,
    is_running: bool,
    networks: Vec<String>,
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

    // General group (autostart)
    let general_group = adw::PreferencesGroup::new();
    general_group.set_title("General");

    let autostart_row = adw::SwitchRow::new();
    autostart_row.set_title("Autostart");
    autostart_row.set_subtitle("Start this VM when the host boots");
    autostart_row.set_active(autostart);
    general_group.add(&autostart_row);

    overview_page.add(&general_group);

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

        let changes = ConfigChanges {
            vcpus: cpu_row.value() as u32,
            memory_mib: memory_row.value() as u64,
            cpu_mode: mode,
            cpu_model: model,
            boot_order: boot_order.clone(),
            autostart: autostart_row.is_active(),
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
        row.set_subtitle(&disk.source_file.clone().unwrap_or_else(|| "No source".to_string()));

        let remove_btn = gtk::Button::from_icon_name("user-trash-symbolic");
        remove_btn.add_css_class("flat");
        remove_btn.set_valign(gtk::Align::Center);
        let on_action_disk = on_action.clone();
        let target = disk.target_dev.clone();
        let window_ref = window.clone();
        remove_btn.connect_clicked(move |_| {
            on_action_disk(ConfigAction::RemoveDisk(target.clone()));
            window_ref.close();
        });
        row.add_suffix(&remove_btn);
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

    window.add(&devices_page);

    window.present();
}
