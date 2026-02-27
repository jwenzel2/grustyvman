use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;

pub fn show_create_snapshot_dialog(
    parent: &adw::ApplicationWindow,
    on_create: impl Fn(String, String) + 'static,
) {
    let dialog = adw::Window::builder()
        .title("Create Snapshot")
        .modal(true)
        .transient_for(parent)
        .default_width(400)
        .default_height(350)
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
    group.set_title("Snapshot Settings");

    // Name field with default timestamp-based name
    let name_row = adw::EntryRow::new();
    name_row.set_title("Name");
    let default_name = generate_default_name();
    name_row.set_text(&default_name);
    group.add(&name_row);

    content.append(&group);

    // Description field
    let desc_group = adw::PreferencesGroup::new();
    desc_group.set_title("Description");

    let desc_frame = gtk::Frame::new(None);
    let desc_scrolled = gtk::ScrolledWindow::new();
    desc_scrolled.set_min_content_height(80);
    desc_scrolled.set_max_content_height(120);
    let desc_view = gtk::TextView::new();
    desc_view.set_wrap_mode(gtk::WrapMode::WordChar);
    desc_view.set_top_margin(8);
    desc_view.set_bottom_margin(8);
    desc_view.set_left_margin(8);
    desc_view.set_right_margin(8);
    desc_scrolled.set_child(Some(&desc_view));
    desc_frame.set_child(Some(&desc_scrolled));
    desc_group.add(&desc_frame);

    content.append(&desc_group);

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
        let buffer = desc_view.buffer();
        let description = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string();

        on_create(name, description);
        if let Some(d) = dialog_weak.upgrade() {
            d.close();
        }
    });

    dialog.present();
}

fn generate_default_name() -> String {
    let now = glib::DateTime::now_local();
    match now {
        Ok(dt) => dt
            .format("snapshot-%Y%m%d-%H%M%S")
            .map(|s| s.to_string())
            .unwrap_or_else(|_| "snapshot".to_string()),
        Err(_) => "snapshot".to_string(),
    }
}
