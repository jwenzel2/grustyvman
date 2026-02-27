use gtk4 as gtk;
use gtk::prelude::*;
use crate::models::vm_object::VmObject;
use crate::ui::vm_row::VmRow;

pub fn create_vm_list_box() -> gtk::ListBox {
    let list_box = gtk::ListBox::new();
    list_box.set_selection_mode(gtk::SelectionMode::Single);
    list_box.add_css_class("navigation-sidebar");
    list_box
}

pub fn create_vm_row_factory(list_box: &gtk::ListBox, model: &gio::ListStore) {
    list_box.bind_model(Some(model), |obj| {
        let vm = obj.downcast_ref::<VmObject>().unwrap();
        let row = VmRow::new();
        row.bind(vm);
        row.upcast()
    });
}
