use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;

pub fn show_create_volume_dialog(
    parent: &adw::ApplicationWindow,
    on_create: impl Fn(String, u64, String) + 'static,
) {
    let dialog = adw::Window::builder()
        .title("Create Volume")
        .modal(true)
        .transient_for(parent)
        .default_width(400)
        .default_height(300)
        .build();

    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    toolbar.add_top_bar(&header);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 24);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(24);
    content.set_margin_end(24);

    let group = adw::PreferencesGroup::new();
    group.set_title("Volume Settings");

    let name_row = adw::EntryRow::new();
    name_row.set_title("Name");
    name_row.set_text("new-volume.qcow2");
    group.add(&name_row);

    let size_adj = gtk::Adjustment::new(20.0, 1.0, 10000.0, 1.0, 10.0, 0.0);
    let size_row = adw::SpinRow::new(Some(&size_adj), 1.0, 0);
    size_row.set_title("Size (GiB)");
    group.add(&size_row);

    let format_row = adw::ComboRow::new();
    format_row.set_title("Format");
    format_row.set_model(Some(&gtk::StringList::new(&["qcow2", "raw"])));
    format_row.set_selected(0);
    group.add(&format_row);

    content.append(&group);

    // Buttons
    let button_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    button_box.set_halign(gtk::Align::End);
    button_box.set_margin_top(12);

    let cancel_btn = gtk::Button::with_label("Cancel");
    let create_btn = gtk::Button::with_label("Create");
    create_btn.add_css_class("suggested-action");

    button_box.append(&cancel_btn);
    button_box.append(&create_btn);
    content.append(&button_box);

    toolbar.set_content(Some(&content));
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
        if name.is_empty() {
            return;
        }
        let size_gib = size_row.value() as u64;
        let capacity_bytes = size_gib * 1024 * 1024 * 1024;

        let formats = ["qcow2", "raw"];
        let format = formats[format_row.selected() as usize].to_string();

        on_create(name, capacity_bytes, format);
        if let Some(d) = dialog_weak.upgrade() {
            d.close();
        }
    });

    dialog.present();
}
