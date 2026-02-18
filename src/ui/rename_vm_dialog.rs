use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;

pub fn show_rename_vm_dialog(
    parent: &adw::ApplicationWindow,
    current_name: &str,
    on_rename: impl Fn(String) + 'static,
) {
    let dialog = gtk::Window::new();
    dialog.set_title(Some("Rename VM"));
    dialog.set_default_size(380, 200);
    dialog.set_modal(true);
    dialog.set_transient_for(Some(parent));

    let toolbar_view = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    toolbar_view.add_top_bar(&header);

    let clamp = adw::Clamp::new();
    clamp.set_maximum_size(360);
    clamp.set_margin_top(24);
    clamp.set_margin_bottom(24);
    clamp.set_margin_start(12);
    clamp.set_margin_end(12);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 20);

    let group = adw::PreferencesGroup::new();
    group.set_title("New Name");

    let name_row = adw::EntryRow::new();
    name_row.set_title("VM Name");
    name_row.set_text(current_name);
    name_row.set_show_apply_button(false);
    group.add(&name_row);

    content.append(&group);

    let rename_btn = gtk::Button::with_label("Rename");
    rename_btn.add_css_class("suggested-action");
    rename_btn.add_css_class("pill");
    rename_btn.set_halign(gtk::Align::Center);
    rename_btn.set_margin_top(12);
    content.append(&rename_btn);

    clamp.set_child(Some(&content));
    toolbar_view.set_content(Some(&clamp));
    dialog.set_child(Some(&toolbar_view));

    let dialog_ref = dialog.clone();
    rename_btn.connect_clicked(move |_| {
        let new_name = name_row.text().trim().to_string();
        if !new_name.is_empty() {
            on_rename(new_name);
            dialog_ref.close();
        }
    });

    dialog.present();
}
