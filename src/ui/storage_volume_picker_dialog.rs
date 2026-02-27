use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::backend::types::VolumeInfo;

fn format_vol_size(bytes: u64) -> String {
    if bytes == 0 {
        return "—".to_string();
    }
    if bytes >= 1 << 30 {
        format!("{:.1} GiB", bytes as f64 / (1u64 << 30) as f64)
    } else {
        format!("{:.0} MiB", bytes as f64 / (1u64 << 20) as f64)
    }
}

fn fill_volume_list(listbox: &gtk::ListBox, volumes: &[VolumeInfo]) {
    while let Some(child) = listbox.first_child() {
        listbox.remove(&child);
    }
    for vol in volumes {
        let row = adw::ActionRow::new();
        row.set_title(&vol.name);
        row.set_subtitle(&format!("{}  —  {}", format_vol_size(vol.capacity), vol.path));
        row.set_activatable(true);
        listbox.append(&row);
    }
}

pub fn show_storage_volume_picker(
    parent: &adw::ApplicationWindow,
    pool_volumes: &[(String, Vec<VolumeInfo>)],
    on_select: impl Fn(String) + 'static,
) {
    if pool_volumes.is_empty() {
        return;
    }

    let on_select: Rc<dyn Fn(String)> = Rc::new(on_select);
    let pool_data: Rc<Vec<(String, Vec<VolumeInfo>)>> = Rc::new(pool_volumes.to_vec());

    // Default to the "default" pool if present, otherwise the first pool.
    let initial_idx = pool_data
        .iter()
        .position(|(n, _)| n == "default")
        .unwrap_or(0);

    let dialog = gtk::Window::new();
    dialog.set_title(Some("Select Storage Volume"));
    dialog.set_default_size(520, 420);
    dialog.set_decorated(false);  // suppress WM title bar; adw::HeaderBar provides the only bar
    dialog.set_modal(true);
    dialog.set_transient_for(Some(parent));

    let toolbar_view = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    toolbar_view.add_top_bar(&header);

    let outer_box = gtk::Box::new(gtk::Orientation::Vertical, 0);

    // Pool selector
    let pool_names: Vec<&str> = pool_data.iter().map(|(n, _)| n.as_str()).collect();
    let pool_list = gtk::StringList::new(&pool_names);
    let pool_combo = adw::ComboRow::new();
    pool_combo.set_title("Storage Pool");
    pool_combo.set_model(Some(&pool_list));
    pool_combo.set_selected(initial_idx as u32);

    let pool_group = adw::PreferencesGroup::new();
    pool_group.add(&pool_combo);
    pool_group.set_margin_top(12);
    pool_group.set_margin_start(12);
    pool_group.set_margin_end(12);
    pool_group.set_margin_bottom(8);
    outer_box.append(&pool_group);

    // Volume listbox inside a scrolled window
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_margin_start(12);
    scroll.set_margin_end(12);

    let listbox = gtk::ListBox::new();
    listbox.add_css_class("boxed-list");
    listbox.set_selection_mode(gtk::SelectionMode::Single);
    scroll.set_child(Some(&listbox));
    outer_box.append(&scroll);

    // Select button
    let select_btn = gtk::Button::with_label("Select");
    select_btn.add_css_class("suggested-action");
    select_btn.add_css_class("pill");
    select_btn.set_halign(gtk::Align::Center);
    select_btn.set_margin_top(12);
    select_btn.set_margin_bottom(16);
    select_btn.set_sensitive(false);
    outer_box.append(&select_btn);

    toolbar_view.set_content(Some(&outer_box));
    dialog.set_child(Some(&toolbar_view));

    // Populate with initial pool
    if let Some((_, volumes)) = pool_data.get(initial_idx) {
        fill_volume_list(&listbox, volumes);
    }

    // Pool combo change → repopulate list
    {
        let listbox = listbox.clone();
        let pool_data = pool_data.clone();
        let select_btn = select_btn.clone();
        pool_combo.connect_selected_notify(move |combo| {
            let idx = combo.selected() as usize;
            select_btn.set_sensitive(false);
            match pool_data.get(idx) {
                Some((_, volumes)) => fill_volume_list(&listbox, volumes),
                None => fill_volume_list(&listbox, &[]),
            }
        });
    }

    // Track the currently selected volume path
    let selected_path: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    // Row selection → update selected_path and enable button
    {
        let pool_data = pool_data.clone();
        let pool_combo = pool_combo.clone();
        let selected_path = selected_path.clone();
        let select_btn = select_btn.clone();
        listbox.connect_row_selected(move |_, opt_row| {
            if let Some(row) = opt_row {
                let pool_idx = pool_combo.selected() as usize;
                let row_idx = row.index() as usize;
                if let Some((_, vols)) = pool_data.get(pool_idx) {
                    if let Some(vol) = vols.get(row_idx) {
                        *selected_path.borrow_mut() = Some(vol.path.clone());
                        select_btn.set_sensitive(true);
                        return;
                    }
                }
            }
            *selected_path.borrow_mut() = None;
            select_btn.set_sensitive(false);
        });
    }

    // Row activation (double-click / Enter)
    {
        let pool_data = pool_data.clone();
        let pool_combo = pool_combo.clone();
        let on_select = on_select.clone();
        let dialog_ref = dialog.clone();
        listbox.connect_row_activated(move |_, row| {
            let pool_idx = pool_combo.selected() as usize;
            let row_idx = row.index() as usize;
            if let Some((_, vols)) = pool_data.get(pool_idx) {
                if let Some(vol) = vols.get(row_idx) {
                    on_select(vol.path.clone());
                    dialog_ref.close();
                }
            }
        });
    }

    // Select button click
    {
        let selected_path = selected_path.clone();
        let on_select = on_select.clone();
        let dialog_ref = dialog.clone();
        select_btn.connect_clicked(move |_| {
            if let Some(path) = selected_path.borrow().clone() {
                on_select(path);
                dialog_ref.close();
            }
        });
    }

    dialog.present();
}
