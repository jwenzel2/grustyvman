use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::backend::domain_xml::NewVmParams;

pub fn show_creation_dialog(
    parent: &adw::ApplicationWindow,
    on_create: impl Fn(NewVmParams) + 'static,
) {
    let dialog = gtk::Window::new();
    dialog.set_title(Some("New Virtual Machine"));
    dialog.set_default_size(480, 520);
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
    iso_row.add_suffix(&browse_btn);

    let clear_btn = gtk::Button::from_icon_name("edit-clear-symbolic");
    clear_btn.set_valign(gtk::Align::Center);
    clear_btn.set_tooltip_text(Some("Clear ISO selection"));
    clear_btn.set_visible(false);
    iso_row.add_suffix(&clear_btn);

    iso_group.add(&iso_row);
    content.append(&iso_group);

    // Browse button handler
    let iso_path_clone = iso_path.clone();
    let iso_row_clone = iso_row.clone();
    let clear_btn_clone = clear_btn.clone();
    let dialog_ref = dialog.clone();
    browse_btn.connect_clicked(move |_| {
        let file_dialog = gtk::FileDialog::new();
        file_dialog.set_title("Select ISO Image");

        let filter = gtk::FileFilter::new();
        filter.add_pattern("*.iso");
        filter.add_pattern("*.ISO");
        filter.set_name(Some("ISO Images"));

        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        file_dialog.set_filters(Some(&filters));

        let iso_path = iso_path_clone.clone();
        let iso_row = iso_row_clone.clone();
        let clear_btn = clear_btn_clone.clone();

        file_dialog.open(
            Some(&dialog_ref),
            gio::Cancellable::NONE,
            move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        let path_str = path.to_string_lossy().to_string();
                        iso_row.set_subtitle(&path_str);
                        *iso_path.borrow_mut() = Some(path_str);
                        clear_btn.set_visible(true);
                    }
                }
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
        let params = NewVmParams {
            name: name_row.text().to_string(),
            vcpus: cpu_row.value() as u32,
            memory_mib: memory_row.value() as u64,
            disk_size_gib: disk_row.value() as u64,
            iso_path: iso_path.borrow().clone(),
        };

        if params.name.is_empty() {
            return;
        }

        on_create(params);
        dialog_ref.close();
    });

    dialog.present();
}
