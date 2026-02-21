use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;

use crate::backend::types::HostdevInfo;

pub fn show_add_hostdev_dialog(
    parent: &adw::ApplicationWindow,
    on_add: impl Fn(HostdevInfo) + 'static,
) {
    let dialog = gtk::Window::new();
    dialog.set_title(Some("Add Host Device"));
    dialog.set_default_size(480, 480);
    dialog.set_decorated(false);
    dialog.set_modal(true);
    dialog.set_transient_for(Some(parent));

    let toolbar_view = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    toolbar_view.add_top_bar(&header);

    let clamp = adw::Clamp::new();
    clamp.set_maximum_size(460);
    clamp.set_margin_top(24);
    clamp.set_margin_bottom(24);
    clamp.set_margin_start(12);
    clamp.set_margin_end(12);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 20);

    // Device type selector
    let type_group = adw::PreferencesGroup::new();
    type_group.set_title("Device Type");

    let type_list = gtk::StringList::new(&["PCI Device", "USB Device"]);
    let type_row = adw::ComboRow::new();
    type_row.set_title("Type");
    type_row.set_model(Some(&type_list));
    type_group.add(&type_row);
    content.append(&type_group);

    // PCI device group
    let pci_group = adw::PreferencesGroup::new();
    pci_group.set_title("PCI Devices");

    let pci_devices = crate::backend::nodedev::list_pci_devices();
    let pci_labels: Vec<&str> = pci_devices.iter().map(|d| d.display_name.as_str()).collect();
    let pci_empty = pci_labels.is_empty();
    let pci_list = gtk::StringList::new(if pci_empty { &["No PCI devices found"] } else { &pci_labels });
    let pci_row = adw::ComboRow::new();
    pci_row.set_title("Device");
    pci_row.set_model(Some(&pci_list));
    pci_group.add(&pci_row);

    // USB device group
    let usb_group = adw::PreferencesGroup::new();
    usb_group.set_title("USB Devices");

    let usb_devices = crate::backend::nodedev::list_usb_devices();
    let usb_labels: Vec<&str> = usb_devices.iter().map(|d| d.display_name.as_str()).collect();
    let usb_empty = usb_labels.is_empty();
    let usb_list = gtk::StringList::new(if usb_empty { &["No USB devices found"] } else { &usb_labels });
    let usb_row = adw::ComboRow::new();
    usb_row.set_title("Device");
    usb_row.set_model(Some(&usb_list));
    usb_group.add(&usb_row);

    content.append(&pci_group);
    content.append(&usb_group);

    // Initially show PCI, hide USB
    usb_group.set_visible(false);

    let pci_group_ref = pci_group.clone();
    let usb_group_ref = usb_group.clone();
    type_row.connect_selected_notify(move |row| {
        let show_pci = row.selected() == 0;
        pci_group_ref.set_visible(show_pci);
        usb_group_ref.set_visible(!show_pci);
    });

    // Add button
    let add_btn = gtk::Button::with_label("Add Device");
    add_btn.add_css_class("suggested-action");
    add_btn.add_css_class("pill");
    add_btn.set_halign(gtk::Align::Center);
    add_btn.set_margin_top(12);
    if pci_empty {
        add_btn.set_sensitive(false);
    }
    content.append(&add_btn);

    clamp.set_child(Some(&content));
    toolbar_view.set_content(Some(&clamp));
    dialog.set_child(Some(&toolbar_view));

    let dialog_ref = dialog.clone();

    // Update button sensitivity when type changes
    {
        let add_btn_ref = add_btn.clone();
        let type_row_ref = type_row.clone();
        let pci_empty_c = pci_empty;
        let usb_empty_c = usb_empty;
        type_row.connect_selected_notify(move |_| {
            let is_pci = type_row_ref.selected() == 0;
            add_btn_ref.set_sensitive(if is_pci { !pci_empty_c } else { !usb_empty_c });
        });
    }

    add_btn.connect_clicked(move |_| {
        let is_pci = type_row.selected() == 0;
        let info = if is_pci {
            let idx = pci_row.selected() as usize;
            pci_devices.get(idx).cloned()
        } else {
            let idx = usb_row.selected() as usize;
            usb_devices.get(idx).cloned()
        };

        if let Some(info) = info {
            on_add(info);
            dialog_ref.close();
        }
    });

    dialog.present();
}
