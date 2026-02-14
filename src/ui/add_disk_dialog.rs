use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::backend::types::{DomainDetails, NewDiskParams};

pub fn show_add_disk_dialog(
    parent: &adw::ApplicationWindow,
    details: &DomainDetails,
    on_add: impl Fn(NewDiskParams) + 'static,
) {
    let dialog = gtk::Window::new();
    dialog.set_title(Some("Add Disk"));
    dialog.set_default_size(480, 500);
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

    // Type group
    let type_group = adw::PreferencesGroup::new();
    type_group.set_title("Disk Type");

    let type_list = gtk::StringList::new(&["Disk", "CD-ROM"]);
    let type_row = adw::ComboRow::new();
    type_row.set_title("Device Type");
    type_row.set_model(Some(&type_list));
    type_group.add(&type_row);

    content.append(&type_group);

    // Source group
    let source_group = adw::PreferencesGroup::new();
    source_group.set_title("Image Source");

    let create_new_row = adw::SwitchRow::new();
    create_new_row.set_title("Create New Image");
    create_new_row.set_subtitle("Create a new qcow2 disk image");
    create_new_row.set_active(true);
    source_group.add(&create_new_row);

    let name_row = adw::EntryRow::new();
    name_row.set_title("Image Name");
    // Auto-populate with a sensible default
    let next_dev = guess_next_dev(details);
    name_row.set_text(&format!("{}-{}.qcow2", details.name, next_dev));
    source_group.add(&name_row);

    let size_row = adw::SpinRow::with_range(1.0, 1000.0, 1.0);
    size_row.set_title("Size (GiB)");
    size_row.set_value(20.0);
    source_group.add(&size_row);

    // File chooser row (hidden when create_new is true)
    let file_path: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let file_row = adw::ActionRow::new();
    file_row.set_title("Image File");
    file_row.set_subtitle("No file selected");
    file_row.set_visible(false);

    let browse_btn = gtk::Button::with_label("Browse...");
    browse_btn.set_valign(gtk::Align::Center);
    file_row.add_suffix(&browse_btn);
    source_group.add(&file_row);

    let name_row_ref = name_row.clone();
    let size_row_ref = size_row.clone();
    let file_row_ref = file_row.clone();
    create_new_row.connect_active_notify(move |row| {
        let active = row.is_active();
        name_row_ref.set_visible(active);
        size_row_ref.set_visible(active);
        file_row_ref.set_visible(!active);
    });

    let file_path_clone = file_path.clone();
    let file_row_clone = file_row.clone();
    let dialog_ref = dialog.clone();
    browse_btn.connect_clicked(move |_| {
        let file_dialog = gtk::FileDialog::new();
        file_dialog.set_title("Select Disk Image");

        let filter = gtk::FileFilter::new();
        filter.add_pattern("*.qcow2");
        filter.add_pattern("*.raw");
        filter.add_pattern("*.img");
        filter.add_pattern("*.iso");
        filter.set_name(Some("Disk Images"));

        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        file_dialog.set_filters(Some(&filters));

        let fp = file_path_clone.clone();
        let fr = file_row_clone.clone();
        file_dialog.open(
            Some(&dialog_ref),
            gio::Cancellable::NONE,
            move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        let path_str = path.to_string_lossy().to_string();
                        fr.set_subtitle(&path_str);
                        *fp.borrow_mut() = Some(path_str);
                    }
                }
            },
        );
    });

    content.append(&source_group);

    // Bus/target group
    let bus_group = adw::PreferencesGroup::new();
    bus_group.set_title("Configuration");

    let bus_list = gtk::StringList::new(&["virtio", "sata", "scsi"]);
    let bus_row = adw::ComboRow::new();
    bus_row.set_title("Bus Type");
    bus_row.set_model(Some(&bus_list));
    bus_group.add(&bus_row);

    let target_row = adw::EntryRow::new();
    target_row.set_title("Target Device");
    target_row.set_text(&next_dev);
    bus_group.add(&target_row);

    content.append(&bus_group);

    // Add button
    let add_btn = gtk::Button::with_label("Add Disk");
    add_btn.add_css_class("suggested-action");
    add_btn.add_css_class("pill");
    add_btn.set_halign(gtk::Align::Center);
    add_btn.set_margin_top(12);
    content.append(&add_btn);

    clamp.set_child(Some(&content));
    toolbar_view.set_content(Some(&clamp));
    dialog.set_child(Some(&toolbar_view));

    let dialog_ref = dialog.clone();
    add_btn.connect_clicked(move |_| {
        let create_new = create_new_row.is_active();
        let device_type = if type_row.selected() == 0 { "disk" } else { "cdrom" };

        let source_file = if create_new {
            let img_name = name_row.text().to_string();
            if img_name.is_empty() {
                return;
            }
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            format!("{home}/.local/share/libvirt/images/{img_name}")
        } else {
            match file_path.borrow().clone() {
                Some(p) => p,
                None => return,
            }
        };

        let bus_idx = bus_row.selected() as usize;
        let bus = ["virtio", "sata", "scsi"][bus_idx].to_string();

        let driver_type = if device_type == "cdrom" && !create_new {
            "raw".to_string()
        } else {
            "qcow2".to_string()
        };

        let target_dev = target_row.text().to_string();
        if target_dev.is_empty() {
            return;
        }

        let params = NewDiskParams {
            source_file,
            target_dev,
            bus,
            device_type: device_type.to_string(),
            driver_type,
            create_new,
            size_gib: size_row.value() as u64,
        };

        on_add(params);
        dialog_ref.close();
    });

    dialog.present();
}

fn guess_next_dev(details: &DomainDetails) -> String {
    let mut max_virtio = 0u8;
    let mut max_sd = 0u8;
    for disk in &details.disks {
        if disk.target_dev.starts_with("vd") {
            if let Some(c) = disk.target_dev.chars().nth(2) {
                let idx = c as u8 - b'a' + 1;
                max_virtio = max_virtio.max(idx);
            }
        } else if disk.target_dev.starts_with("sd") {
            if let Some(c) = disk.target_dev.chars().nth(2) {
                let idx = c as u8 - b'a' + 1;
                max_sd = max_sd.max(idx);
            }
        }
    }
    if max_virtio > 0 || max_sd == 0 {
        let next = (b'a' + max_virtio) as char;
        format!("vd{next}")
    } else {
        let next = (b'a' + max_sd) as char;
        format!("sd{next}")
    }
}
