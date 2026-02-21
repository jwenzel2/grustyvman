use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub fn show_upload_volume_dialog(
    parent: &adw::ApplicationWindow,
    on_upload: impl Fn(String, String) + 'static,
) {
    let dialog = adw::Window::builder()
        .title("Upload Image")
        .modal(true)
        .transient_for(parent)
        .default_width(420)
        .default_height(280)
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
    group.set_title("Image File");

    // File picker row
    let file_row = adw::ActionRow::new();
    file_row.set_title("Source File");
    file_row.set_subtitle("No file selected");
    file_row.set_activatable(false);

    let browse_btn = gtk::Button::with_label("Browse…");
    browse_btn.set_valign(gtk::Align::Center);
    browse_btn.add_css_class("flat");
    file_row.add_suffix(&browse_btn);
    group.add(&file_row);

    // Volume name entry
    let name_row = adw::EntryRow::new();
    name_row.set_title("Volume Name");
    group.add(&name_row);

    content.append(&group);

    // Buttons
    let button_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    button_box.set_halign(gtk::Align::End);
    button_box.set_margin_top(12);

    let cancel_btn = gtk::Button::with_label("Cancel");
    let upload_btn = gtk::Button::with_label("Upload");
    upload_btn.add_css_class("suggested-action");
    upload_btn.set_sensitive(false);

    button_box.append(&cancel_btn);
    button_box.append(&upload_btn);
    content.append(&button_box);

    toolbar.set_content(Some(&content));
    dialog.set_content(Some(&toolbar));

    // State
    let selected_path: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    // File dialog
    let selected_path_browse = selected_path.clone();
    let file_row_ref = file_row.clone();
    let name_row_ref = name_row.clone();
    let upload_btn_ref = upload_btn.clone();
    let dialog_ref = dialog.clone();
    browse_btn.connect_clicked(move |_| {
        let file_dialog = gtk::FileDialog::new();
        file_dialog.set_title("Select Image File");

        let filter = gtk::FileFilter::new();
        filter.add_pattern("*.iso");
        filter.add_pattern("*.img");
        filter.add_pattern("*.qcow2");
        filter.set_name(Some("Disk Images (*.iso, *.img, *.qcow2)"));

        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        file_dialog.set_filters(Some(&filters));

        let sp = selected_path_browse.clone();
        let fr = file_row_ref.clone();
        let nr = name_row_ref.clone();
        let ub = upload_btn_ref.clone();
        file_dialog.open(Some(&dialog_ref), gio::Cancellable::NONE, move |result| {
            if let Ok(file) = result {
                if let Some(path) = file.path() {
                    let path_str = path.to_string_lossy().to_string();
                    // Auto-fill volume name from filename
                    if let Some(fname) = path.file_name() {
                        let name = fname.to_string_lossy().to_string();
                        if nr.text().is_empty() || nr.text() == "" {
                            nr.set_text(&name);
                        }
                        // Always update if it was the same as the previous filename
                        nr.set_text(&name);
                    }
                    fr.set_subtitle(&path_str);
                    *sp.borrow_mut() = Some(path_str);
                    ub.set_sensitive(true);
                }
            }
        });
    });

    // Cancel
    let dialog_weak = dialog.downgrade();
    cancel_btn.connect_clicked(move |_| {
        if let Some(d) = dialog_weak.upgrade() {
            d.close();
        }
    });

    // Upload
    let dialog_weak = dialog.downgrade();
    upload_btn.connect_clicked(move |_| {
        let src = match selected_path.borrow().clone() {
            Some(p) => p,
            None => return,
        };
        let vol_name = name_row.text().to_string();
        if vol_name.is_empty() {
            return;
        }
        on_upload(src, vol_name);
        if let Some(d) = dialog_weak.upgrade() {
            d.close();
        }
    });

    dialog.present();
}
