use glib::prelude::*;
use glib::subclass::prelude::*;
use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::cell::RefCell;

use crate::backend;
use crate::models::vm_object::VmObject;
use crate::ui::vm_details_view::VmDetailsView;
use crate::ui::vm_list_view;

fn spawn_blocking<F, T>(f: F) -> async_channel::Receiver<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let result = f();
        let _ = tx.send_blocking(result);
    });
    rx
}

mod imp {
    use super::*;

    pub struct Window {
        pub split_view: adw::NavigationSplitView,
        pub list_store: gio::ListStore,
        pub content_stack: gtk::Stack,
        pub details_view: VmDetailsView,
        pub toast_overlay: adw::ToastOverlay,
        pub connection_uri: RefCell<String>,
        pub selected_uuid: RefCell<Option<String>>,
        pub content_title: adw::WindowTitle,
        pub btn_start: gtk::Button,
        pub btn_pause: gtk::Button,
        pub btn_stop: gtk::Button,
        pub btn_force_stop: gtk::Button,
        pub btn_reboot: gtk::Button,
        pub btn_console: gtk::Button,
        pub btn_delete: gtk::Button,
        pub btn_settings: gtk::Button,
    }

    impl Default for Window {
        fn default() -> Self {
            Self {
                split_view: adw::NavigationSplitView::new(),
                list_store: gio::ListStore::new::<VmObject>(),
                content_stack: gtk::Stack::new(),
                details_view: VmDetailsView::new(),
                toast_overlay: adw::ToastOverlay::new(),
                connection_uri: RefCell::new("qemu:///system".to_string()),
                selected_uuid: RefCell::new(None),
                content_title: adw::WindowTitle::new("", ""),
                btn_start: gtk::Button::new(),
                btn_pause: gtk::Button::new(),
                btn_stop: gtk::Button::new(),
                btn_force_stop: gtk::Button::new(),
                btn_reboot: gtk::Button::new(),
                btn_console: gtk::Button::new(),
                btn_delete: gtk::Button::new(),
                btn_settings: gtk::Button::new(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "GrustyvmanWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;
    }

    impl ObjectImpl for Window {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.set_title(Some("Grustyvman"));
            obj.set_default_size(1000, 700);
            obj.setup_ui();
        }
    }

    impl gtk::subclass::prelude::WidgetImpl for Window {}
    impl gtk::subclass::prelude::WindowImpl for Window {}
    impl gtk::subclass::prelude::ApplicationWindowImpl for Window {}
    impl adw::subclass::prelude::AdwApplicationWindowImpl for Window {}
}

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl Window {
    pub fn new(app: &adw::Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    fn setup_ui(&self) {
        let imp = self.imp();

        // --- CSS ---
        let css_provider = gtk::CssProvider::new();
        css_provider.load_from_string(
            r#"
            .status-dot { font-size: 14px; }
            .status-running .status-dot,
            .status-running { color: #2ec27e; }
            .status-paused .status-dot,
            .status-paused { color: #f5c211; }
            .status-shutoff .status-dot,
            .status-shutoff { color: @insensitive_fg_color; }
            .status-crashed .status-dot,
            .status-crashed { color: #e01b24; }
            "#,
        );
        gtk::style_context_add_provider_for_display(
            &gtk::prelude::WidgetExt::display(self),
            &css_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        // --- Sidebar ---
        let sidebar_toolbar = adw::ToolbarView::new();
        let sidebar_header = adw::HeaderBar::new();

        let conn_dropdown = gtk::DropDown::from_strings(&["qemu:///system", "qemu:///session"]);
        conn_dropdown.set_selected(0);
        sidebar_header.set_title_widget(Some(&conn_dropdown));

        let new_vm_btn = gtk::Button::from_icon_name("list-add-symbolic");
        new_vm_btn.set_tooltip_text(Some("New Virtual Machine"));
        sidebar_header.pack_end(&new_vm_btn);

        sidebar_toolbar.add_top_bar(&sidebar_header);

        let list_box = vm_list_view::create_vm_list_box();
        vm_list_view::create_vm_row_factory(&list_box, &imp.list_store);

        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_vexpand(true);
        scrolled.set_child(Some(&list_box));
        sidebar_toolbar.set_content(Some(&scrolled));

        let sidebar_page = adw::NavigationPage::new(&sidebar_toolbar, "Virtual Machines");

        // --- Content ---
        let content_toolbar = adw::ToolbarView::new();
        let content_header = adw::HeaderBar::new();

        let content_title = &imp.content_title;
        content_header.set_title_widget(Some(content_title));

        let btn_start = &imp.btn_start;
        btn_start.set_icon_name("media-playback-start-symbolic");
        btn_start.set_tooltip_text(Some("Start"));
        btn_start.set_sensitive(false);

        let btn_pause = &imp.btn_pause;
        btn_pause.set_icon_name("media-playback-pause-symbolic");
        btn_pause.set_tooltip_text(Some("Pause / Resume"));
        btn_pause.set_sensitive(false);

        let btn_stop = &imp.btn_stop;
        btn_stop.set_icon_name("media-playback-stop-symbolic");
        btn_stop.set_tooltip_text(Some("Shut Down"));
        btn_stop.set_sensitive(false);

        let btn_force_stop = &imp.btn_force_stop;
        btn_force_stop.set_icon_name("process-stop-symbolic");
        btn_force_stop.set_tooltip_text(Some("Force Stop"));
        btn_force_stop.add_css_class("destructive-action");
        btn_force_stop.set_sensitive(false);

        let btn_reboot = &imp.btn_reboot;
        btn_reboot.set_icon_name("view-refresh-symbolic");
        btn_reboot.set_tooltip_text(Some("Reboot"));
        btn_reboot.set_sensitive(false);

        let btn_console = &imp.btn_console;
        btn_console.set_icon_name("utilities-terminal-symbolic");
        btn_console.set_tooltip_text(Some("Console"));
        btn_console.set_sensitive(false);

        let btn_delete = &imp.btn_delete;
        btn_delete.set_icon_name("user-trash-symbolic");
        btn_delete.set_tooltip_text(Some("Delete"));
        btn_delete.add_css_class("destructive-action");
        btn_delete.set_sensitive(false);

        let btn_settings = &imp.btn_settings;
        btn_settings.set_icon_name("emblem-system-symbolic");
        btn_settings.set_tooltip_text(Some("Settings"));
        btn_settings.set_sensitive(false);

        content_header.pack_start(btn_start);
        content_header.pack_start(btn_pause);
        content_header.pack_start(btn_stop);
        content_header.pack_start(btn_force_stop);
        content_header.pack_start(btn_reboot);
        content_header.pack_end(btn_settings);
        content_header.pack_end(btn_delete);
        content_header.pack_end(btn_console);

        content_toolbar.add_top_bar(&content_header);

        let stack = &imp.content_stack;

        let empty_page = adw::StatusPage::new();
        empty_page.set_title("Select a Virtual Machine");
        empty_page.set_description(Some("Choose a VM from the sidebar to view its details"));
        empty_page.set_icon_name(Some("computer-symbolic"));
        stack.add_named(&empty_page, Some("empty"));

        let details_scrolled = gtk::ScrolledWindow::new();
        let details_clamp = adw::Clamp::new();
        details_clamp.set_maximum_size(800);
        details_clamp.set_child(Some(&imp.details_view.container));
        details_scrolled.set_child(Some(&details_clamp));
        stack.add_named(&details_scrolled, Some("details"));

        stack.set_visible_child_name("empty");
        content_toolbar.set_content(Some(stack));

        let content_page = adw::NavigationPage::new(&content_toolbar, "Details");

        // --- Split view ---
        imp.split_view.set_sidebar(Some(&sidebar_page));
        imp.split_view.set_content(Some(&content_page));
        imp.split_view.set_min_sidebar_width(260.0);
        imp.split_view.set_max_sidebar_width(360.0);

        imp.toast_overlay.set_child(Some(&imp.split_view));
        self.set_content(Some(&imp.toast_overlay));

        // --- Signals ---

        // Connection dropdown
        let win = self.downgrade();
        conn_dropdown.connect_selected_notify(move |dropdown| {
            if let Some(win) = win.upgrade() {
                let uris = ["qemu:///system", "qemu:///session"];
                let idx = dropdown.selected() as usize;
                if idx < uris.len() {
                    *win.imp().connection_uri.borrow_mut() = uris[idx].to_string();
                    *win.imp().selected_uuid.borrow_mut() = None;
                    win.imp().content_stack.set_visible_child_name("empty");
                    win.imp().content_title.set_title("");
                    win.imp().content_title.set_subtitle("");
                    win.update_button_sensitivity(None);
                    win.refresh_vm_list();
                }
            }
        });

        // VM list selection
        let win = self.downgrade();
        list_box.connect_row_selected(move |_, row| {
            if let Some(win) = win.upgrade() {
                if let Some(row) = row {
                    let idx = row.index() as u32;
                    if let Some(obj) = win.imp().list_store.item(idx) {
                        let vm = obj.downcast_ref::<VmObject>().unwrap();
                        let uuid = vm.uuid();
                        *win.imp().selected_uuid.borrow_mut() = Some(uuid.clone());
                        win.imp().content_title.set_title(&vm.name());
                        win.imp().content_title.set_subtitle(&vm.state());
                        win.load_vm_details(&uuid);
                    }
                } else {
                    *win.imp().selected_uuid.borrow_mut() = None;
                    win.imp().content_stack.set_visible_child_name("empty");
                    win.imp().content_title.set_title("");
                    win.imp().content_title.set_subtitle("");
                    win.update_button_sensitivity(None);
                }
            }
        });

        // New VM button
        let win = self.downgrade();
        new_vm_btn.connect_clicked(move |_| {
            if let Some(win) = win.upgrade() {
                win.show_create_vm_dialog();
            }
        });

        self.connect_action_buttons();

        // Auto-refresh timer
        let win = self.downgrade();
        glib::timeout_add_seconds_local(5, move || {
            if let Some(win) = win.upgrade() {
                win.refresh_vm_list();
                glib::ControlFlow::Continue
            } else {
                glib::ControlFlow::Break
            }
        });

        // Initial refresh
        self.refresh_vm_list();
    }

    fn connect_action_buttons(&self) {
        let imp = self.imp();

        let win = self.downgrade();
        imp.btn_start.connect_clicked(move |_| {
            if let Some(win) = win.upgrade() {
                win.do_vm_action("start");
            }
        });

        let win = self.downgrade();
        imp.btn_pause.connect_clicked(move |_| {
            if let Some(win) = win.upgrade() {
                let uuid = win.imp().selected_uuid.borrow().clone();
                if let Some(uuid) = uuid {
                    let state = win.get_selected_vm_state(&uuid);
                    match state.as_deref() {
                        Some("Running") => win.do_vm_action("pause"),
                        Some("Paused") => win.do_vm_action("resume"),
                        _ => {}
                    }
                }
            }
        });

        let win = self.downgrade();
        imp.btn_stop.connect_clicked(move |_| {
            if let Some(win) = win.upgrade() {
                win.do_vm_action("shutdown");
            }
        });

        let win = self.downgrade();
        imp.btn_force_stop.connect_clicked(move |_| {
            if let Some(win) = win.upgrade() {
                win.confirm_and_act(
                    "Force Stop VM?",
                    "This will immediately power off the VM. Unsaved data may be lost.",
                    "Force Stop",
                    "force_stop",
                );
            }
        });

        let win = self.downgrade();
        imp.btn_reboot.connect_clicked(move |_| {
            if let Some(win) = win.upgrade() {
                win.do_vm_action("reboot");
            }
        });

        let win = self.downgrade();
        imp.btn_console.connect_clicked(move |_| {
            if let Some(win) = win.upgrade() {
                win.do_vm_action("console");
            }
        });

        let win = self.downgrade();
        imp.btn_delete.connect_clicked(move |_| {
            if let Some(win) = win.upgrade() {
                win.confirm_and_act(
                    "Delete VM?",
                    "This will permanently remove the VM definition. Disk images will not be deleted.",
                    "Delete",
                    "delete",
                );
            }
        });

        let win = self.downgrade();
        imp.btn_settings.connect_clicked(move |_| {
            if let Some(win) = win.upgrade() {
                win.show_config_dialog();
            }
        });
    }

    fn get_selected_vm_state(&self, uuid: &str) -> Option<String> {
        let store = &self.imp().list_store;
        for i in 0..store.n_items() {
            if let Some(obj) = store.item(i) {
                let vm = obj.downcast_ref::<VmObject>().unwrap();
                if vm.uuid() == uuid {
                    return Some(vm.state());
                }
            }
        }
        None
    }

    fn confirm_and_act(&self, title: &str, body: &str, button_label: &str, action: &str) {
        let dialog = adw::MessageDialog::new(Some(self), Some(title), Some(body));
        dialog.add_response("cancel", "Cancel");
        dialog.add_response("confirm", button_label);
        dialog.set_response_appearance("confirm", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");

        let win = self.downgrade();
        let action = action.to_string();
        dialog.connect_response(None, move |_, response| {
            if response == "confirm" {
                if let Some(win) = win.upgrade() {
                    win.do_vm_action(&action);
                }
            }
        });

        dialog.present();
    }

    fn do_vm_action(&self, action: &str) {
        let uuid = self.imp().selected_uuid.borrow().clone();
        let uri = self.imp().connection_uri.borrow().clone();

        let Some(uuid) = uuid else { return };

        let win = self.downgrade();
        let action = action.to_string();

        let rx = spawn_blocking({
            let uuid = uuid.clone();
            let uri = uri.clone();
            let action = action.clone();
            move || match action.as_str() {
                "start" => backend::domain::start_vm(&uri, &uuid),
                "shutdown" => backend::domain::shutdown_vm(&uri, &uuid),
                "force_stop" => backend::domain::force_stop_vm(&uri, &uuid),
                "pause" => backend::domain::pause_vm(&uri, &uuid),
                "resume" => backend::domain::resume_vm(&uri, &uuid),
                "reboot" => backend::domain::reboot_vm(&uri, &uuid),
                "delete" => backend::domain::delete_vm(&uri, &uuid),
                "console" => backend::domain::launch_console(&uri, &uuid),
                _ => Ok(()),
            }
        });

        glib::spawn_future_local(async move {
            let Ok(result) = rx.recv().await else { return };
            let Some(win) = win.upgrade() else { return };

            match result {
                Ok(()) => {
                    let msg = match action.as_str() {
                        "start" => "VM started",
                        "shutdown" => "Shutdown signal sent",
                        "force_stop" => "VM force stopped",
                        "pause" => "VM paused",
                        "resume" => "VM resumed",
                        "reboot" => "Reboot signal sent",
                        "delete" => {
                            *win.imp().selected_uuid.borrow_mut() = None;
                            win.imp().content_stack.set_visible_child_name("empty");
                            win.imp().content_title.set_title("");
                            win.imp().content_title.set_subtitle("");
                            win.update_button_sensitivity(None);
                            "VM deleted"
                        }
                        "console" => "Console launched",
                        _ => "Done",
                    };
                    win.show_toast(msg);
                    win.refresh_vm_list();
                }
                Err(e) => {
                    win.show_toast(&format!("Error: {e}"));
                }
            }
        });
    }

    fn show_toast(&self, message: &str) {
        let toast = adw::Toast::new(message);
        toast.set_timeout(3);
        self.imp().toast_overlay.add_toast(toast);
    }

    fn refresh_vm_list(&self) {
        let uri = self.imp().connection_uri.borrow().clone();
        let win = self.downgrade();

        let rx = spawn_blocking(move || backend::connection::list_all_vms(&uri));

        glib::spawn_future_local(async move {
            let Ok(result) = rx.recv().await else { return };
            let Some(win) = win.upgrade() else { return };

            match result {
                Ok(vms) => {
                    win.update_vm_list(&vms);
                }
                Err(e) => {
                    log::error!("Failed to list VMs: {e}");
                }
            }
        });
    }

    fn update_vm_list(&self, vms: &[backend::types::VmInfo]) {
        let store = &self.imp().list_store;
        let selected_uuid = self.imp().selected_uuid.borrow().clone();

        // Build a map of existing objects by UUID
        let mut existing: std::collections::HashMap<String, (u32, VmObject)> =
            std::collections::HashMap::new();
        for i in 0..store.n_items() {
            if let Some(obj) = store.item(i) {
                let vm = obj.downcast_ref::<VmObject>().unwrap();
                existing.insert(vm.uuid(), (i, vm.clone()));
            }
        }

        let new_uuids: std::collections::HashSet<String> =
            vms.iter().map(|v| v.uuid.clone()).collect();

        // Remove VMs that no longer exist
        let mut to_remove: Vec<u32> = existing
            .iter()
            .filter(|(uuid, _)| !new_uuids.contains(*uuid))
            .map(|(_, (idx, _))| *idx)
            .collect();
        to_remove.sort_unstable_by(|a, b| b.cmp(a));
        for idx in to_remove {
            store.remove(idx);
        }

        // Update existing or add new
        for vm_info in vms {
            if let Some((_, obj)) = existing.get(&vm_info.uuid) {
                obj.update_from(vm_info);
            } else {
                store.append(&VmObject::new(vm_info));
            }
        }

        // Update button sensitivity for selected VM
        if let Some(ref uuid) = selected_uuid {
            let state = vms.iter().find(|v| v.uuid == *uuid).map(|v| v.state);
            self.update_button_sensitivity(state);

            if let Some(vm_info) = vms.iter().find(|v| v.uuid == *uuid) {
                self.imp().content_title.set_subtitle(vm_info.state.label());
            }
        }
    }

    fn update_button_sensitivity(&self, state: Option<backend::types::VmState>) {
        use crate::backend::types::VmState;
        let imp = self.imp();

        let (start, pause, stop, force, reboot, console, delete, settings) = match state {
            Some(VmState::Running) => (false, true, true, true, true, true, false, true),
            Some(VmState::Paused) => (false, true, false, true, false, false, false, true),
            Some(VmState::Shutoff) => (true, false, false, false, false, false, true, true),
            Some(VmState::Crashed) => (false, false, false, true, false, false, true, true),
            Some(_) => (false, false, false, true, false, false, false, true),
            None => (false, false, false, false, false, false, false, false),
        };

        imp.btn_start.set_sensitive(start);
        imp.btn_pause.set_sensitive(pause);
        imp.btn_stop.set_sensitive(stop);
        imp.btn_force_stop.set_sensitive(force);
        imp.btn_reboot.set_sensitive(reboot);
        imp.btn_console.set_sensitive(console);
        imp.btn_delete.set_sensitive(delete);
        imp.btn_settings.set_sensitive(settings);
    }

    fn load_vm_details(&self, uuid: &str) {
        let uri = self.imp().connection_uri.borrow().clone();
        let uuid = uuid.to_string();
        let win = self.downgrade();

        let rx = spawn_blocking({
            let uri = uri.clone();
            let uuid = uuid.clone();
            move || {
                let xml = backend::domain::get_domain_xml(&uri, &uuid)?;
                let details = backend::domain_xml::parse_domain_xml(&xml)?;
                let autostart = backend::domain::get_autostart(&uri, &uuid)?;
                let vms = backend::connection::list_all_vms(&uri)?;
                let vm_info = vms.into_iter().find(|v| v.uuid == uuid);
                Ok::<_, crate::error::AppError>((details, vm_info, autostart))
            }
        });

        glib::spawn_future_local(async move {
            let Ok(result) = rx.recv().await else { return };
            let Some(win) = win.upgrade() else { return };

            match result {
                Ok((details, vm_info, autostart)) => {
                    let state_label = vm_info
                        .as_ref()
                        .map(|v| v.state.label())
                        .unwrap_or("Unknown");
                    let domain_id = vm_info.as_ref().and_then(|v| v.id);

                    win.imp().details_view.update(&details, state_label, domain_id, autostart);
                    win.imp().content_stack.set_visible_child_name("details");

                    let state = vm_info.map(|v| v.state);
                    win.update_button_sensitivity(state);
                }
                Err(e) => {
                    win.show_toast(&format!("Failed to load details: {e}"));
                }
            }
        });
    }

    fn show_create_vm_dialog(&self) {
        let win = self.downgrade();
        crate::ui::vm_creation_dialog::show_creation_dialog(self.upcast_ref(), move |params| {
            let Some(win) = win.upgrade() else { return };
            let uri = win.imp().connection_uri.borrow().clone();

            let rx = spawn_blocking(move || backend::domain_xml::create_vm(&uri, &params));

            let win2 = win.downgrade();
            glib::spawn_future_local(async move {
                let Ok(result) = rx.recv().await else { return };
                let Some(win) = win2.upgrade() else { return };

                match result {
                    Ok(()) => {
                        win.show_toast("VM created successfully");
                        win.refresh_vm_list();
                    }
                    Err(e) => {
                        win.show_toast(&format!("Failed to create VM: {e}"));
                    }
                }
            });
        });
    }

    fn show_config_dialog(&self) {
        let uuid = self.imp().selected_uuid.borrow().clone();
        let Some(uuid) = uuid else { return };

        let uri = self.imp().connection_uri.borrow().clone();
        let win = self.downgrade();

        // Async fetch all needed data
        let rx = spawn_blocking({
            let uri = uri.clone();
            let uuid = uuid.clone();
            move || {
                let xml = backend::domain::get_domain_xml(&uri, &uuid)?;
                let details = backend::domain_xml::parse_domain_xml(&xml)?;
                let autostart = backend::domain::get_autostart(&uri, &uuid)?;
                let networks = backend::domain::list_networks(&uri).unwrap_or_default();
                let vms = backend::connection::list_all_vms(&uri)?;
                let is_running = vms.iter().any(|v| {
                    v.uuid == uuid && v.state == backend::types::VmState::Running
                });
                Ok::<_, crate::error::AppError>((details, autostart, networks, is_running))
            }
        });

        glib::spawn_future_local(async move {
            let Ok(result) = rx.recv().await else { return };
            let Some(win) = win.upgrade() else { return };

            match result {
                Ok((details, autostart, networks, is_running)) => {
                    let win_ref = win.downgrade();
                    let uuid_clone = uuid.clone();
                    let uri = win.imp().connection_uri.borrow().clone();

                    crate::ui::vm_config_dialog::show_config_dialog(
                        win.upcast_ref(),
                        &details,
                        autostart,
                        is_running,
                        networks,
                        move |action| {
                            let Some(win) = win_ref.upgrade() else { return };
                            let uri = uri.clone();
                            let uuid = uuid_clone.clone();

                            let rx = spawn_blocking({
                                let uri = uri.clone();
                                let uuid = uuid.clone();
                                move || {
                                    Self::handle_config_action(&uri, &uuid, action)
                                }
                            });

                            let win2 = win.downgrade();
                            let uuid2 = uuid.clone();
                            glib::spawn_future_local(async move {
                                let Ok(result) = rx.recv().await else { return };
                                let Some(win) = win2.upgrade() else { return };

                                match result {
                                    Ok(()) => {
                                        win.show_toast("Configuration updated");
                                        win.refresh_vm_list();
                                        win.load_vm_details(&uuid2);
                                    }
                                    Err(e) => {
                                        win.show_toast(&format!("Failed to update config: {e}"));
                                    }
                                }
                            });
                        },
                    );
                }
                Err(e) => {
                    win.show_toast(&format!("Failed to load config: {e}"));
                }
            }
        });
    }

    fn handle_config_action(
        uri: &str,
        uuid: &str,
        action: backend::types::ConfigAction,
    ) -> Result<(), crate::error::AppError> {
        use backend::types::ConfigAction;

        match action {
            ConfigAction::ApplyGeneral(changes) => {
                // Set autostart
                backend::domain::set_autostart(uri, uuid, changes.autostart)?;

                let xml = backend::domain::get_domain_xml(uri, uuid)?;

                // Apply vcpu/memory changes (only if non-zero, i.e. from Overview page)
                let xml = if changes.vcpus > 0 && changes.memory_mib > 0 {
                    backend::domain_xml::modify_domain_xml(&xml, changes.vcpus, changes.memory_mib)?
                } else {
                    xml
                };

                // Apply CPU model changes
                let xml = backend::domain_xml::modify_cpu_model(
                    &xml,
                    changes.cpu_mode,
                    changes.cpu_model.as_deref(),
                )?;

                // Apply boot order
                let xml = backend::domain_xml::modify_boot_order(&xml, &changes.boot_order)?;

                backend::domain::update_domain_xml(uri, &xml)?;
                Ok(())
            }
            ConfigAction::AddDisk(params) => {
                if params.create_new {
                    backend::domain::create_disk_image(&params.source_file, params.size_gib)?;
                }
                let xml = backend::domain::get_domain_xml(uri, uuid)?;
                let xml = backend::domain_xml::add_disk_device(&xml, &params)?;
                backend::domain::update_domain_xml(uri, &xml)?;
                Ok(())
            }
            ConfigAction::RemoveDisk(target_dev) => {
                let xml = backend::domain::get_domain_xml(uri, uuid)?;
                let xml = backend::domain_xml::remove_disk_device(&xml, &target_dev)?;
                backend::domain::update_domain_xml(uri, &xml)?;
                Ok(())
            }
            ConfigAction::AddNetwork(params) => {
                let xml = backend::domain::get_domain_xml(uri, uuid)?;
                let xml = backend::domain_xml::add_network_device(&xml, &params)?;
                backend::domain::update_domain_xml(uri, &xml)?;
                Ok(())
            }
            ConfigAction::RemoveNetwork(mac) => {
                let xml = backend::domain::get_domain_xml(uri, uuid)?;
                let xml = backend::domain_xml::remove_network_device(&xml, &mac)?;
                backend::domain::update_domain_xml(uri, &xml)?;
                Ok(())
            }
            ConfigAction::SetAutostart(enabled) => {
                backend::domain::set_autostart(uri, uuid, enabled)?;
                Ok(())
            }
        }
    }
}
