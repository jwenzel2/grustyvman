use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;

use crate::backend::types::PoolCreateParams;

const POOL_TYPES: &[(&str, &str)] = &[
    ("dir", "Filesystem Directory"),
    ("fs", "Pre-Formatted Block Device"),
    ("netfs", "Network Filesystem (NFS)"),
    ("logical", "LVM Volume Group"),
    ("iscsi", "iSCSI Target"),
    ("disk", "Physical Disk Device"),
];

pub fn show_create_pool_dialog(
    parent: &adw::ApplicationWindow,
    on_create: impl Fn(String, String, PoolCreateParams) + 'static,
) {
    let dialog = adw::Window::builder()
        .title("Create Storage Pool")
        .modal(true)
        .transient_for(parent)
        .default_width(500)
        .default_height(450)
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

    // --- General group ---
    let general_group = adw::PreferencesGroup::new();
    general_group.set_title("Pool Settings");

    let name_row = adw::EntryRow::new();
    name_row.set_title("Name");
    name_row.set_text("new-pool");
    general_group.add(&name_row);

    let type_labels: Vec<&str> = POOL_TYPES.iter().map(|(_, label)| *label).collect();
    let type_row = adw::ComboRow::new();
    type_row.set_title("Type");
    type_row.set_model(Some(&gtk::StringList::new(&type_labels)));
    type_row.set_selected(0);
    general_group.add(&type_row);

    content.append(&general_group);

    // --- Source group (shown for non-dir types) ---
    let source_group = adw::PreferencesGroup::new();
    source_group.set_title("Source");
    source_group.set_visible(false);

    let device_row = adw::EntryRow::new();
    device_row.set_title("Device Path");
    device_row.set_text("/dev/sdb1");
    source_group.add(&device_row);

    let host_row = adw::EntryRow::new();
    host_row.set_title("Hostname");
    source_group.add(&host_row);

    let source_dir_row = adw::EntryRow::new();
    source_dir_row.set_title("Source Path");
    source_group.add(&source_dir_row);

    let source_name_row = adw::EntryRow::new();
    source_name_row.set_title("Source Name");
    source_group.add(&source_name_row);

    let format_row = adw::ComboRow::new();
    format_row.set_title("Format");
    source_group.add(&format_row);

    content.append(&source_group);

    // --- Target group ---
    let target_group = adw::PreferencesGroup::new();
    target_group.set_title("Target");

    let path_row = adw::EntryRow::new();
    path_row.set_title("Target Path");
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    path_row.set_text(&format!("{home}/.local/share/libvirt/pools/new-pool"));
    target_group.add(&path_row);

    let browse_btn = gtk::Button::from_icon_name("folder-open-symbolic");
    browse_btn.set_tooltip_text(Some("Browse"));
    browse_btn.set_valign(gtk::Align::Center);
    path_row.add_suffix(&browse_btn);

    let dialog_weak = dialog.downgrade();
    let path_row_clone = path_row.clone();
    browse_btn.connect_clicked(move |_| {
        let Some(dialog_ref) = dialog_weak.upgrade() else { return };
        let file_dialog = gtk::FileDialog::new();
        file_dialog.set_title("Select Directory");

        let path_row_inner = path_row_clone.clone();
        file_dialog.select_folder(Some(&dialog_ref), None::<&gio::Cancellable>, move |result| {
            if let Ok(file) = result {
                if let Some(path) = file.path() {
                    path_row_inner.set_text(&path.to_string_lossy());
                }
            }
        });
    });

    content.append(&target_group);

    // --- Update path when name changes (only for dir type) ---
    let path_row_clone = path_row.clone();
    let type_row_clone = type_row.clone();
    name_row.connect_changed(move |entry| {
        let name = entry.text();
        let idx = type_row_clone.selected() as usize;
        let pool_type = POOL_TYPES.get(idx).map(|(t, _)| *t).unwrap_or("dir");
        if !name.is_empty() && pool_type == "dir" {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            path_row_clone.set_text(&format!("{home}/.local/share/libvirt/pools/{name}"));
        }
    });

    // --- Update visible fields when type changes ---
    {
        let source_group = source_group.clone();
        let device_row = device_row.clone();
        let host_row = host_row.clone();
        let source_dir_row = source_dir_row.clone();
        let source_name_row = source_name_row.clone();
        let format_row = format_row.clone();
        let path_row = path_row.clone();
        let browse_btn = browse_btn.clone();

        type_row.connect_selected_notify(move |combo| {
            let idx = combo.selected() as usize;
            let pool_type = POOL_TYPES.get(idx).map(|(t, _)| *t).unwrap_or("dir");

            // Reset visibility
            device_row.set_visible(false);
            host_row.set_visible(false);
            source_dir_row.set_visible(false);
            source_name_row.set_visible(false);
            format_row.set_visible(false);

            match pool_type {
                "dir" => {
                    source_group.set_visible(false);
                    browse_btn.set_visible(true);
                    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
                    path_row.set_text(&format!("{home}/.local/share/libvirt/pools/new-pool"));
                }
                "fs" => {
                    source_group.set_visible(true);
                    device_row.set_visible(true);
                    device_row.set_title("Device Path");
                    device_row.set_text("/dev/sdb1");
                    format_row.set_visible(true);
                    format_row.set_model(Some(&gtk::StringList::new(&["auto", "ext4", "xfs", "btrfs", "ext3"])));
                    format_row.set_selected(0);
                    browse_btn.set_visible(true);
                    path_row.set_text("/mnt/pool");
                }
                "netfs" => {
                    source_group.set_visible(true);
                    host_row.set_visible(true);
                    source_dir_row.set_visible(true);
                    source_dir_row.set_title("Source Path");
                    source_dir_row.set_text("/export/share");
                    format_row.set_visible(true);
                    format_row.set_model(Some(&gtk::StringList::new(&["nfs", "glusterfs", "cifs"])));
                    format_row.set_selected(0);
                    browse_btn.set_visible(true);
                    path_row.set_text("/mnt/nfs-pool");
                }
                "logical" => {
                    source_group.set_visible(true);
                    device_row.set_visible(true);
                    device_row.set_title("Device Path");
                    device_row.set_text("/dev/sdb");
                    source_name_row.set_visible(true);
                    source_name_row.set_title("Volume Group Name");
                    source_name_row.set_text("my-vg");
                    browse_btn.set_visible(false);
                    path_row.set_text("/dev/my-vg");
                }
                "iscsi" => {
                    source_group.set_visible(true);
                    host_row.set_visible(true);
                    device_row.set_visible(true);
                    device_row.set_title("Target IQN");
                    device_row.set_text("iqn.2025-01.com.example:storage");
                    browse_btn.set_visible(false);
                    path_row.set_text("/dev/disk/by-path");
                }
                "disk" => {
                    source_group.set_visible(true);
                    device_row.set_visible(true);
                    device_row.set_title("Device Path");
                    device_row.set_text("/dev/sdb");
                    format_row.set_visible(true);
                    format_row.set_model(Some(&gtk::StringList::new(&["gpt", "dos", "mac", "bsd", "sun"])));
                    format_row.set_selected(0);
                    browse_btn.set_visible(false);
                    path_row.set_text("/dev");
                }
                _ => {
                    source_group.set_visible(false);
                    browse_btn.set_visible(true);
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
        let target_path = path_row.text().to_string();
        if name.is_empty() || target_path.is_empty() {
            return;
        }

        let idx = type_row.selected() as usize;
        let pool_type = POOL_TYPES.get(idx).map(|(t, _)| *t).unwrap_or("dir");

        let source_format = if format_row.is_visible() {
            let model = format_row.model().unwrap();
            let sl = model.downcast_ref::<gtk::StringList>().unwrap();
            let sel = format_row.selected() as u32;
            sl.string(sel).map(|s| s.to_string()).unwrap_or_default()
        } else {
            String::new()
        };

        let params = PoolCreateParams {
            target_path,
            source_device: device_row.text().to_string(),
            source_host: host_row.text().to_string(),
            source_dir: source_dir_row.text().to_string(),
            source_name: source_name_row.text().to_string(),
            source_format,
        };

        on_create(name, pool_type.to_string(), params);
        if let Some(d) = dialog_weak.upgrade() {
            d.close();
        }
    });

    dialog.present();
}
