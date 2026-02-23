use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::rc::Rc;

use crate::backend::types::HostdevInfo;

pub fn show_add_hostdev_dialog(
    parent: &adw::ApplicationWindow,
    on_add: impl Fn(HostdevInfo) + 'static,
) {
    let dialog = gtk::Window::new();
    dialog.set_title(Some("Add Host Device"));
    dialog.set_default_size(560, 520);
    dialog.set_decorated(false);
    dialog.set_modal(true);
    dialog.set_transient_for(Some(parent));

    let toolbar_view = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    toolbar_view.add_top_bar(&header);

    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    outer.set_margin_top(16);
    outer.set_margin_bottom(16);
    outer.set_margin_start(16);
    outer.set_margin_end(16);
    outer.set_spacing(12);

    // Device type selector
    let type_group = adw::PreferencesGroup::new();
    type_group.set_title("Device Type");

    let type_list = gtk::StringList::new(&["PCI Device", "USB Device"]);
    let type_row = adw::ComboRow::new();
    type_row.set_title("Type");
    type_row.set_model(Some(&type_list));
    type_group.add(&type_row);
    outer.append(&type_group);

    // --- PCI section ---
    let pci_label = gtk::Label::new(Some("PCI Devices"));
    pci_label.set_halign(gtk::Align::Start);
    pci_label.add_css_class("heading");
    outer.append(&pci_label);

    let pci_devices = crate::backend::nodedev::list_pci_devices();

    let pci_scroll = gtk::ScrolledWindow::new();
    pci_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    pci_scroll.set_vexpand(true);
    pci_scroll.set_min_content_height(220);
    pci_scroll.add_css_class("card");

    let pci_listbox = gtk::ListBox::new();
    pci_listbox.set_selection_mode(gtk::SelectionMode::Single);
    pci_listbox.add_css_class("boxed-list");

    if pci_devices.is_empty() {
        let row = gtk::ListBoxRow::new();
        let lbl = gtk::Label::new(Some("No PCI devices found"));
        lbl.set_margin_top(8);
        lbl.set_margin_bottom(8);
        lbl.set_margin_start(12);
        lbl.set_margin_end(12);
        lbl.add_css_class("dim-label");
        row.set_child(Some(&lbl));
        row.set_selectable(false);
        pci_listbox.append(&row);
    } else {
        for dev in &pci_devices {
            let row = gtk::ListBoxRow::new();
            let lbl = gtk::Label::new(Some(&dev.display_name));
            lbl.set_halign(gtk::Align::Start);
            lbl.set_wrap(true);
            lbl.set_xalign(0.0);
            lbl.set_margin_top(8);
            lbl.set_margin_bottom(8);
            lbl.set_margin_start(12);
            lbl.set_margin_end(12);
            row.set_child(Some(&lbl));
            pci_listbox.append(&row);
        }
        // Select first by default
        if let Some(first) = pci_listbox.row_at_index(0) {
            pci_listbox.select_row(Some(&first));
        }
    }

    pci_scroll.set_child(Some(&pci_listbox));
    outer.append(&pci_scroll);

    // --- USB section ---
    let usb_label = gtk::Label::new(Some("USB Devices"));
    usb_label.set_halign(gtk::Align::Start);
    usb_label.add_css_class("heading");
    outer.append(&usb_label);

    let usb_devices = crate::backend::nodedev::list_usb_devices();

    let usb_scroll = gtk::ScrolledWindow::new();
    usb_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    usb_scroll.set_vexpand(true);
    usb_scroll.set_min_content_height(220);
    usb_scroll.add_css_class("card");

    let usb_listbox = gtk::ListBox::new();
    usb_listbox.set_selection_mode(gtk::SelectionMode::Single);
    usb_listbox.add_css_class("boxed-list");

    if usb_devices.is_empty() {
        let row = gtk::ListBoxRow::new();
        let lbl = gtk::Label::new(Some("No USB devices found"));
        lbl.set_margin_top(8);
        lbl.set_margin_bottom(8);
        lbl.set_margin_start(12);
        lbl.set_margin_end(12);
        lbl.add_css_class("dim-label");
        row.set_child(Some(&lbl));
        row.set_selectable(false);
        usb_listbox.append(&row);
    } else {
        for dev in &usb_devices {
            let row = gtk::ListBoxRow::new();
            let lbl = gtk::Label::new(Some(&dev.display_name));
            lbl.set_halign(gtk::Align::Start);
            lbl.set_wrap(true);
            lbl.set_xalign(0.0);
            lbl.set_margin_top(8);
            lbl.set_margin_bottom(8);
            lbl.set_margin_start(12);
            lbl.set_margin_end(12);
            row.set_child(Some(&lbl));
            usb_listbox.append(&row);
        }
        if let Some(first) = usb_listbox.row_at_index(0) {
            usb_listbox.select_row(Some(&first));
        }
    }

    usb_scroll.set_child(Some(&usb_listbox));
    outer.append(&usb_scroll);

    // Initially show PCI, hide USB
    usb_label.set_visible(false);
    usb_scroll.set_visible(false);

    let pci_label_ref = pci_label.clone();
    let pci_scroll_ref = pci_scroll.clone();
    let usb_label_ref = usb_label.clone();
    let usb_scroll_ref = usb_scroll.clone();
    type_row.connect_selected_notify(move |row| {
        let show_pci = row.selected() == 0;
        pci_label_ref.set_visible(show_pci);
        pci_scroll_ref.set_visible(show_pci);
        usb_label_ref.set_visible(!show_pci);
        usb_scroll_ref.set_visible(!show_pci);
    });

    // Add button
    let add_btn = gtk::Button::with_label("Add Device");
    add_btn.add_css_class("suggested-action");
    add_btn.add_css_class("pill");
    add_btn.set_halign(gtk::Align::Center);
    add_btn.set_margin_top(4);
    if pci_devices.is_empty() {
        add_btn.set_sensitive(false);
    }
    outer.append(&add_btn);

    toolbar_view.set_content(Some(&outer));
    dialog.set_child(Some(&toolbar_view));

    // Update button sensitivity on type change
    {
        let add_btn_ref = add_btn.clone();
        let type_row_ref = type_row.clone();
        let pci_empty = pci_devices.is_empty();
        let usb_empty = usb_devices.is_empty();
        type_row.connect_selected_notify(move |_| {
            let is_pci = type_row_ref.selected() == 0;
            add_btn_ref.set_sensitive(if is_pci { !pci_empty } else { !usb_empty });
        });
    }

    let pci_devices = Rc::new(pci_devices);
    let usb_devices = Rc::new(usb_devices);
    let dialog_ref = dialog.clone();

    add_btn.connect_clicked(move |_| {
        let is_pci = type_row.selected() == 0;
        let info = if is_pci {
            pci_listbox
                .selected_row()
                .map(|r| r.index() as usize)
                .and_then(|idx| pci_devices.get(idx).cloned())
        } else {
            usb_listbox
                .selected_row()
                .map(|r| r.index() as usize)
                .and_then(|idx| usb_devices.get(idx).cloned())
        };

        if let Some(info) = info {
            on_add(info);
            dialog_ref.close();
        }
    });

    dialog.present();
}
