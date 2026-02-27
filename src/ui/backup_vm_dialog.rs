use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;

/// Show a file-save dialog for choosing the backup destination.
pub fn show_backup_vm_dialog(
    parent: &adw::ApplicationWindow,
    vm_name: &str,
    on_backup: impl Fn(String) + 'static,
) {
    let file_dialog = gtk::FileDialog::new();
    file_dialog.set_title("Save VM Backup");
    file_dialog.set_initial_name(Some(&format!("{vm_name}-backup.tar.gz")));

    let filter = gtk::FileFilter::new();
    filter.add_pattern("*.tar.gz");
    filter.set_name(Some("Backup Archives (*.tar.gz)"));

    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    file_dialog.set_filters(Some(&filters));

    file_dialog.save(Some(parent), gio::Cancellable::NONE, move |result| {
        if let Ok(file) = result {
            if let Some(path) = file.path() {
                on_backup(path.to_string_lossy().to_string());
            }
        }
    });
}

/// Show a file-open dialog for choosing a backup archive to restore.
pub fn show_restore_vm_dialog(
    parent: &adw::ApplicationWindow,
    on_restore: impl Fn(String) + 'static,
) {
    let file_dialog = gtk::FileDialog::new();
    file_dialog.set_title("Restore VM from Backup");

    let filter = gtk::FileFilter::new();
    filter.add_pattern("*.tar.gz");
    filter.set_name(Some("Backup Archives (*.tar.gz)"));

    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    file_dialog.set_filters(Some(&filters));

    file_dialog.open(Some(parent), gio::Cancellable::NONE, move |result| {
        if let Ok(file) = result {
            if let Some(path) = file.path() {
                on_restore(path.to_string_lossy().to_string());
            }
        }
    });
}
