use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;

pub struct CloneParams {
    pub new_name: String,
    pub full_clone: bool,
}

pub fn show_clone_vm_dialog(
    parent: &adw::ApplicationWindow,
    source_name: &str,
    on_clone: impl Fn(CloneParams) + 'static,
) {
    let dialog = gtk::Window::new();
    dialog.set_title(Some("Clone VM"));
    dialog.set_default_size(400, 280);
    dialog.set_decorated(false);
    dialog.set_modal(true);
    dialog.set_transient_for(Some(parent));

    let toolbar_view = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    toolbar_view.add_top_bar(&header);

    let clamp = adw::Clamp::new();
    clamp.set_maximum_size(380);
    clamp.set_margin_top(24);
    clamp.set_margin_bottom(24);
    clamp.set_margin_start(12);
    clamp.set_margin_end(12);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 20);

    let group = adw::PreferencesGroup::new();
    group.set_title("Clone Configuration");

    let name_row = adw::EntryRow::new();
    name_row.set_title("New VM Name");
    name_row.set_text(&format!("{}-clone", source_name));
    name_row.set_show_apply_button(false);
    group.add(&name_row);

    let clone_type_list = gtk::StringList::new(&["Full Clone (independent copy)", "Linked Clone (uses backing store)"]);
    let clone_type_row = adw::ComboRow::new();
    clone_type_row.set_title("Clone Type");
    clone_type_row.set_model(Some(&clone_type_list));
    group.add(&clone_type_row);

    content.append(&group);

    let clone_btn = gtk::Button::with_label("Clone VM");
    clone_btn.add_css_class("suggested-action");
    clone_btn.add_css_class("pill");
    clone_btn.set_halign(gtk::Align::Center);
    clone_btn.set_margin_top(12);
    content.append(&clone_btn);

    clamp.set_child(Some(&content));
    toolbar_view.set_content(Some(&clamp));
    dialog.set_child(Some(&toolbar_view));

    let dialog_ref = dialog.clone();
    clone_btn.connect_clicked(move |_| {
        let new_name = name_row.text().trim().to_string();
        if new_name.is_empty() {
            return;
        }
        let full_clone = clone_type_row.selected() == 0;
        on_clone(CloneParams { new_name, full_clone });
        dialog_ref.close();
    });

    dialog.present();
}
