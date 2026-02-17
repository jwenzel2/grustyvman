use glib::prelude::*;
use glib::subclass::prelude::*;
use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::cell::RefCell;

use crate::backend;
use crate::backend::types::RawPerfSample;
use crate::models::network_object::NetworkObject;
use crate::models::pool_object::PoolObject;
use crate::models::vm_object::VmObject;
use crate::ui::network_details_view::NetworkDetailsView;
use crate::ui::network_row::NetworkRow;
use crate::ui::pool_details_view::PoolDetailsView;
use crate::ui::pool_row::PoolRow;
use crate::ui::vm_details_view::VmDetailsView;
use crate::ui::vm_performance_view::VmPerformanceView;
use crate::ui::vm_snapshot_view::VmSnapshotView;
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
    use std::time::Instant;

    #[allow(deprecated)]
    pub struct Window {
        pub split_view: adw::NavigationSplitView,
        // VM state
        pub list_store: gio::ListStore,
        pub outer_stack: gtk::Stack,
        pub view_stack: adw::ViewStack,
        pub details_view: VmDetailsView,
        pub perf_view: VmPerformanceView,
        pub snapshot_view: VmSnapshotView,
        pub toast_overlay: adw::ToastOverlay,
        pub connection_uri: RefCell<String>,
        pub selected_uuid: RefCell<Option<String>>,
        pub view_switcher_title: adw::ViewSwitcherTitle,
        pub btn_start: gtk::Button,
        pub btn_pause: gtk::Button,
        pub btn_stop: gtk::Button,
        pub btn_force_stop: gtk::Button,
        pub btn_reboot: gtk::Button,
        pub btn_console: gtk::Button,
        pub btn_delete: gtk::Button,
        pub btn_settings: gtk::Button,
        // Perf sampling state
        pub perf_timer_id: RefCell<Option<glib::SourceId>>,
        pub last_perf_sample: RefCell<Option<(Instant, RawPerfSample)>>,
        pub disk_targets: RefCell<Vec<String>>,
        pub iface_targets: RefCell<Vec<String>>,
        // Storage state
        pub sidebar_stack: gtk::Stack,
        pub pool_list_store: gio::ListStore,
        pub pool_details_view: PoolDetailsView,
        pub selected_pool_uuid: RefCell<Option<String>>,
        pub active_sidebar: RefCell<String>,
        // Network state
        pub network_list_store: gio::ListStore,
        pub network_details_view: NetworkDetailsView,
        pub selected_network_uuid: RefCell<Option<String>>,
    }

    #[allow(deprecated)]
    impl Default for Window {
        fn default() -> Self {
            Self {
                split_view: adw::NavigationSplitView::new(),
                list_store: gio::ListStore::new::<VmObject>(),
                outer_stack: gtk::Stack::new(),
                view_stack: adw::ViewStack::new(),
                details_view: VmDetailsView::new(),
                perf_view: VmPerformanceView::new(),
                snapshot_view: VmSnapshotView::new(),
                toast_overlay: adw::ToastOverlay::new(),
                connection_uri: RefCell::new("qemu:///system".to_string()),
                selected_uuid: RefCell::new(None),
                view_switcher_title: adw::ViewSwitcherTitle::new(),
                btn_start: gtk::Button::new(),
                btn_pause: gtk::Button::new(),
                btn_stop: gtk::Button::new(),
                btn_force_stop: gtk::Button::new(),
                btn_reboot: gtk::Button::new(),
                btn_console: gtk::Button::new(),
                btn_delete: gtk::Button::new(),
                btn_settings: gtk::Button::new(),
                perf_timer_id: RefCell::new(None),
                last_perf_sample: RefCell::new(None),
                disk_targets: RefCell::new(Vec::new()),
                iface_targets: RefCell::new(Vec::new()),
                sidebar_stack: gtk::Stack::new(),
                pool_list_store: gio::ListStore::new::<PoolObject>(),
                pool_details_view: PoolDetailsView::new(),
                selected_pool_uuid: RefCell::new(None),
                active_sidebar: RefCell::new("vms".to_string()),
                network_list_store: gio::ListStore::new::<NetworkObject>(),
                network_details_view: NetworkDetailsView::new(),
                selected_network_uuid: RefCell::new(None),
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

#[allow(deprecated)]
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

        // Sidebar toggle buttons (VMs / Storage)
        let toggle_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        toggle_box.add_css_class("linked");
        toggle_box.set_halign(gtk::Align::Center);
        toggle_box.set_margin_top(6);
        toggle_box.set_margin_bottom(6);
        toggle_box.set_margin_start(6);
        toggle_box.set_margin_end(6);

        let btn_vms = gtk::ToggleButton::with_label("VMs");
        btn_vms.set_active(true);
        btn_vms.set_hexpand(true);
        let btn_storage = gtk::ToggleButton::with_label("Storage");
        btn_storage.set_group(Some(&btn_vms));
        btn_storage.set_hexpand(true);
        let btn_networks = gtk::ToggleButton::with_label("Networks");
        btn_networks.set_group(Some(&btn_vms));
        btn_networks.set_hexpand(true);
        toggle_box.append(&btn_vms);
        toggle_box.append(&btn_storage);
        toggle_box.append(&btn_networks);

        // VM list
        let vm_list_box = vm_list_view::create_vm_list_box();
        vm_list_view::create_vm_row_factory(&vm_list_box, &imp.list_store);

        let vm_scrolled = gtk::ScrolledWindow::new();
        vm_scrolled.set_vexpand(true);
        vm_scrolled.set_child(Some(&vm_list_box));

        // Pool list
        let pool_list_box = gtk::ListBox::new();
        pool_list_box.set_selection_mode(gtk::SelectionMode::Single);
        pool_list_box.add_css_class("navigation-sidebar");
        pool_list_box.bind_model(Some(&imp.pool_list_store), |obj| {
            let pool = obj.downcast_ref::<PoolObject>().unwrap();
            let row = PoolRow::new();
            row.bind(pool);
            row.upcast()
        });

        let pool_scrolled = gtk::ScrolledWindow::new();
        pool_scrolled.set_vexpand(true);
        pool_scrolled.set_child(Some(&pool_list_box));

        // Network list
        let network_list_box = gtk::ListBox::new();
        network_list_box.set_selection_mode(gtk::SelectionMode::Single);
        network_list_box.add_css_class("navigation-sidebar");
        network_list_box.bind_model(Some(&imp.network_list_store), |obj| {
            let network = obj.downcast_ref::<NetworkObject>().unwrap();
            let row = NetworkRow::new();
            row.bind(network);
            row.upcast()
        });

        let network_scrolled = gtk::ScrolledWindow::new();
        network_scrolled.set_vexpand(true);
        network_scrolled.set_child(Some(&network_list_box));

        // Sidebar stack
        let sidebar_stack = &imp.sidebar_stack;
        sidebar_stack.add_named(&vm_scrolled, Some("vms"));
        sidebar_stack.add_named(&pool_scrolled, Some("storage"));
        sidebar_stack.add_named(&network_scrolled, Some("networks"));
        sidebar_stack.set_visible_child_name("vms");

        let sidebar_content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        sidebar_content.append(&toggle_box);
        sidebar_content.append(sidebar_stack);

        sidebar_toolbar.set_content(Some(&sidebar_content));

        let sidebar_page = adw::NavigationPage::new(&sidebar_toolbar, "Grustyvman");

        // --- Content ---
        let content_toolbar = adw::ToolbarView::new();
        let content_header = adw::HeaderBar::new();

        // ViewSwitcherTitle for Details/Performance tabs
        let view_switcher_title = &imp.view_switcher_title;
        view_switcher_title.set_stack(Some(&imp.view_stack));
        content_header.set_title_widget(Some(view_switcher_title));

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

        // Bottom bar fallback for narrow windows
        let view_switcher_bar = adw::ViewSwitcherBar::new();
        view_switcher_bar.set_stack(Some(&imp.view_stack));
        content_toolbar.add_bottom_bar(&view_switcher_bar);

        // Reveal bottom bar when title can't show tabs
        view_switcher_title
            .bind_property("title-visible", &view_switcher_bar, "reveal")
            .sync_create()
            .build();

        // --- ViewStack (Details + Performance) ---
        let view_stack = &imp.view_stack;

        let details_scrolled = gtk::ScrolledWindow::new();
        let details_clamp = adw::Clamp::new();
        details_clamp.set_maximum_size(800);
        details_clamp.set_child(Some(&imp.details_view.container));
        details_scrolled.set_child(Some(&details_clamp));
        let details_page = view_stack.add_titled(&details_scrolled, Some("details"), "Details");
        details_page.set_icon_name(Some("info-symbolic"));

        let perf_scrolled = gtk::ScrolledWindow::new();
        let perf_clamp = adw::Clamp::new();
        perf_clamp.set_maximum_size(800);
        perf_clamp.set_child(Some(&imp.perf_view.container));
        perf_scrolled.set_child(Some(&perf_clamp));
        let perf_page = view_stack.add_titled(&perf_scrolled, Some("performance"), "Performance");
        perf_page.set_icon_name(Some("utilities-system-monitor-symbolic"));

        let snap_page = view_stack.add_titled(&imp.snapshot_view.container, Some("snapshots"), "Snapshots");
        snap_page.set_icon_name(Some("camera-photo-symbolic"));

        // --- Pool content ---
        let pool_scrolled = gtk::ScrolledWindow::new();
        let pool_clamp = adw::Clamp::new();
        pool_clamp.set_maximum_size(800);
        pool_clamp.set_child(Some(&imp.pool_details_view.container));
        pool_scrolled.set_child(Some(&pool_clamp));

        // --- Outer stack ---
        let outer_stack = &imp.outer_stack;

        let empty_page = adw::StatusPage::new();
        empty_page.set_title("Select a Virtual Machine");
        empty_page.set_description(Some("Choose a VM from the sidebar to view its details"));
        empty_page.set_icon_name(Some("computer-symbolic"));
        outer_stack.add_named(&empty_page, Some("empty"));
        outer_stack.add_named(view_stack, Some("vm-content"));
        outer_stack.add_named(&pool_scrolled, Some("pool-content"));

        let pool_empty_page = adw::StatusPage::new();
        pool_empty_page.set_title("Select a Storage Pool");
        pool_empty_page.set_description(Some("Choose a pool from the sidebar to view its details"));
        pool_empty_page.set_icon_name(Some("drive-harddisk-symbolic"));
        outer_stack.add_named(&pool_empty_page, Some("pool-empty"));

        // Network content
        let network_scrolled_content = gtk::ScrolledWindow::new();
        let network_clamp = adw::Clamp::new();
        network_clamp.set_maximum_size(800);
        network_clamp.set_child(Some(&imp.network_details_view.container));
        network_scrolled_content.set_child(Some(&network_clamp));
        outer_stack.add_named(&network_scrolled_content, Some("network-content"));

        let network_empty_page = adw::StatusPage::new();
        network_empty_page.set_title("Select a Virtual Network");
        network_empty_page.set_description(Some("Choose a network from the sidebar to view its details"));
        network_empty_page.set_icon_name(Some("network-wired-symbolic"));
        outer_stack.add_named(&network_empty_page, Some("network-empty"));

        outer_stack.set_visible_child_name("empty");
        content_toolbar.set_content(Some(outer_stack));

        let content_page = adw::NavigationPage::new(&content_toolbar, "Details");

        // --- Split view ---
        imp.split_view.set_sidebar(Some(&sidebar_page));
        imp.split_view.set_content(Some(&content_page));
        imp.split_view.set_min_sidebar_width(260.0);
        imp.split_view.set_max_sidebar_width(360.0);

        imp.toast_overlay.set_child(Some(&imp.split_view));
        self.set_content(Some(&imp.toast_overlay));

        // --- Signals ---

        // Sidebar toggle buttons
        let win = self.downgrade();
        let new_btn_ref = new_vm_btn.clone();
        btn_vms.connect_toggled(move |btn| {
            if btn.is_active() {
                if let Some(win) = win.upgrade() {
                    *win.imp().active_sidebar.borrow_mut() = "vms".to_string();
                    win.imp().sidebar_stack.set_visible_child_name("vms");
                    new_btn_ref.set_tooltip_text(Some("New Virtual Machine"));
                    new_btn_ref.set_icon_name("list-add-symbolic");
                    // Show VM content or empty
                    if win.imp().selected_uuid.borrow().is_some() {
                        win.imp().outer_stack.set_visible_child_name("vm-content");
                    } else {
                        win.imp().outer_stack.set_visible_child_name("empty");
                    }
                    win.update_button_sensitivity_for_mode();
                }
            }
        });

        let win = self.downgrade();
        let new_btn_ref = new_vm_btn.clone();
        btn_storage.connect_toggled(move |btn| {
            if btn.is_active() {
                if let Some(win) = win.upgrade() {
                    *win.imp().active_sidebar.borrow_mut() = "storage".to_string();
                    win.imp().sidebar_stack.set_visible_child_name("storage");
                    new_btn_ref.set_tooltip_text(Some("New Storage Pool"));
                    new_btn_ref.set_icon_name("list-add-symbolic");
                    win.stop_perf_sampling();
                    // Show pool content or pool empty
                    if win.imp().selected_pool_uuid.borrow().is_some() {
                        win.imp().outer_stack.set_visible_child_name("pool-content");
                    } else {
                        win.imp().outer_stack.set_visible_child_name("pool-empty");
                    }
                    win.update_button_sensitivity_for_mode();
                    win.refresh_pool_list();
                }
            }
        });

        let win = self.downgrade();
        let new_btn_ref = new_vm_btn.clone();
        btn_networks.connect_toggled(move |btn| {
            if btn.is_active() {
                if let Some(win) = win.upgrade() {
                    *win.imp().active_sidebar.borrow_mut() = "networks".to_string();
                    win.imp().sidebar_stack.set_visible_child_name("networks");
                    new_btn_ref.set_tooltip_text(Some("New Virtual Network"));
                    new_btn_ref.set_icon_name("list-add-symbolic");
                    win.stop_perf_sampling();
                    if win.imp().selected_network_uuid.borrow().is_some() {
                        win.imp().outer_stack.set_visible_child_name("network-content");
                    } else {
                        win.imp().outer_stack.set_visible_child_name("network-empty");
                    }
                    win.update_button_sensitivity_for_mode();
                    win.refresh_network_list();
                }
            }
        });

        // Connection dropdown
        let win = self.downgrade();
        conn_dropdown.connect_selected_notify(move |dropdown| {
            if let Some(win) = win.upgrade() {
                let uris = ["qemu:///system", "qemu:///session"];
                let idx = dropdown.selected() as usize;
                if idx < uris.len() {
                    *win.imp().connection_uri.borrow_mut() = uris[idx].to_string();
                    *win.imp().selected_uuid.borrow_mut() = None;
                    *win.imp().selected_pool_uuid.borrow_mut() = None;
                    *win.imp().selected_network_uuid.borrow_mut() = None;
                    win.imp().outer_stack.set_visible_child_name("empty");
                    win.imp().view_switcher_title.set_title("");
                    win.imp().view_switcher_title.set_subtitle("");
                    win.update_button_sensitivity(None);
                    win.stop_perf_sampling();
                    win.imp().pool_list_store.remove_all();
                    win.imp().network_list_store.remove_all();
                    win.refresh_vm_list();
                }
            }
        });

        // VM list selection
        let win = self.downgrade();
        vm_list_box.connect_row_selected(move |_, row| {
            if let Some(win) = win.upgrade() {
                if let Some(row) = row {
                    let idx = row.index() as u32;
                    if let Some(obj) = win.imp().list_store.item(idx) {
                        let vm = obj.downcast_ref::<VmObject>().unwrap();
                        let uuid = vm.uuid();
                        *win.imp().selected_uuid.borrow_mut() = Some(uuid.clone());
                        win.imp().view_switcher_title.set_title(&vm.name());
                        win.imp().view_switcher_title.set_subtitle(&vm.state());
                        win.load_vm_details(&uuid);
                    }
                } else {
                    *win.imp().selected_uuid.borrow_mut() = None;
                    win.imp().outer_stack.set_visible_child_name("empty");
                    win.imp().view_switcher_title.set_title("");
                    win.imp().view_switcher_title.set_subtitle("");
                    win.update_button_sensitivity(None);
                    win.stop_perf_sampling();
                }
            }
        });

        // Pool list selection
        let win = self.downgrade();
        pool_list_box.connect_row_selected(move |_, row| {
            if let Some(win) = win.upgrade() {
                if let Some(row) = row {
                    let idx = row.index() as u32;
                    if let Some(obj) = win.imp().pool_list_store.item(idx) {
                        let pool = obj.downcast_ref::<PoolObject>().unwrap();
                        let uuid = pool.uuid();
                        *win.imp().selected_pool_uuid.borrow_mut() = Some(uuid.clone());
                        win.imp().view_switcher_title.set_title(&pool.name());
                        win.imp().view_switcher_title.set_subtitle(&pool.state());
                        win.load_pool_details(&uuid);
                    }
                } else {
                    *win.imp().selected_pool_uuid.borrow_mut() = None;
                    win.imp().outer_stack.set_visible_child_name("pool-empty");
                    win.imp().view_switcher_title.set_title("");
                    win.imp().view_switcher_title.set_subtitle("");
                }
            }
        });

        // Network list selection
        let win = self.downgrade();
        network_list_box.connect_row_selected(move |_, row| {
            if let Some(win) = win.upgrade() {
                if let Some(row) = row {
                    let idx = row.index() as u32;
                    if let Some(obj) = win.imp().network_list_store.item(idx) {
                        let network = obj.downcast_ref::<NetworkObject>().unwrap();
                        let uuid = network.uuid();
                        *win.imp().selected_network_uuid.borrow_mut() = Some(uuid.clone());
                        win.imp().view_switcher_title.set_title(&network.name());
                        win.imp().view_switcher_title.set_subtitle(&network.state());
                        win.load_network_details(&uuid);
                    }
                } else {
                    *win.imp().selected_network_uuid.borrow_mut() = None;
                    win.imp().outer_stack.set_visible_child_name("network-empty");
                    win.imp().view_switcher_title.set_title("");
                    win.imp().view_switcher_title.set_subtitle("");
                }
            }
        });

        // New VM button
        let win = self.downgrade();
        new_vm_btn.connect_clicked(move |_| {
            if let Some(win) = win.upgrade() {
                let sidebar = win.imp().active_sidebar.borrow().clone();
                match sidebar.as_str() {
                    "storage" => win.show_create_pool_dialog(),
                    "networks" => win.show_create_network_dialog(),
                    _ => win.show_create_vm_dialog(),
                }
            }
        });

        self.connect_action_buttons();
        self.connect_pool_action_buttons();
        self.connect_network_action_buttons();
        self.connect_snapshot_callbacks();

        // Auto-refresh timer
        let win = self.downgrade();
        glib::timeout_add_seconds_local(5, move || {
            if let Some(win) = win.upgrade() {
                win.refresh_vm_list();
                let sidebar = win.imp().active_sidebar.borrow().clone();
                match sidebar.as_str() {
                    "storage" => win.refresh_pool_list(),
                    "networks" => win.refresh_network_list(),
                    _ => {}
                }
                glib::ControlFlow::Continue
            } else {
                glib::ControlFlow::Break
            }
        });

        // Initial refresh
        self.refresh_vm_list();
    }

    fn set_vm_buttons_visible(&self, visible: bool) {
        let imp = self.imp();
        imp.btn_start.set_visible(visible);
        imp.btn_pause.set_visible(visible);
        imp.btn_stop.set_visible(visible);
        imp.btn_force_stop.set_visible(visible);
        imp.btn_reboot.set_visible(visible);
        imp.btn_console.set_visible(visible);
        imp.btn_delete.set_visible(visible);
        imp.btn_settings.set_visible(visible);
        imp.view_switcher_title.set_visible(visible);
    }

    fn update_button_sensitivity_for_mode(&self) {
        let sidebar = self.imp().active_sidebar.borrow().clone();
        if sidebar == "storage" || sidebar == "networks" {
            self.set_vm_buttons_visible(false);
        } else {
            self.set_vm_buttons_visible(true);
            // Re-evaluate VM state for buttons
            let uuid = self.imp().selected_uuid.borrow().clone();
            if let Some(uuid) = uuid {
                let state = self.get_selected_vm_state(&uuid);
                let vm_state = state.as_deref().and_then(|s| match s {
                    "Running" => Some(backend::types::VmState::Running),
                    "Paused" => Some(backend::types::VmState::Paused),
                    "Shutoff" => Some(backend::types::VmState::Shutoff),
                    "Crashed" => Some(backend::types::VmState::Crashed),
                    _ => None,
                });
                self.update_button_sensitivity(vm_state);
            } else {
                self.update_button_sensitivity(None);
            }
        }
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

    fn connect_pool_action_buttons(&self) {
        let imp = self.imp();

        // Pool Start
        let win = self.downgrade();
        imp.pool_details_view.btn_start.connect_clicked(move |_| {
            if let Some(win) = win.upgrade() {
                win.do_pool_action("start");
            }
        });

        // Pool Stop
        let win = self.downgrade();
        imp.pool_details_view.btn_stop.connect_clicked(move |_| {
            if let Some(win) = win.upgrade() {
                win.do_pool_action("stop");
            }
        });

        // Pool Refresh
        let win = self.downgrade();
        imp.pool_details_view.btn_refresh.connect_clicked(move |_| {
            if let Some(win) = win.upgrade() {
                win.do_pool_action("refresh");
            }
        });

        // Pool Delete
        let win = self.downgrade();
        imp.pool_details_view.btn_delete.connect_clicked(move |_| {
            if let Some(win) = win.upgrade() {
                let dialog = adw::MessageDialog::new(Some(&win), Some("Delete Pool?"), Some("This will undefine the storage pool."));
                dialog.add_response("cancel", "Cancel");
                dialog.add_response("confirm", "Delete");
                dialog.set_response_appearance("confirm", adw::ResponseAppearance::Destructive);
                dialog.set_default_response(Some("cancel"));
                dialog.set_close_response("cancel");

                let win2 = win.downgrade();
                dialog.connect_response(None, move |_, response| {
                    if response == "confirm" {
                        if let Some(win) = win2.upgrade() {
                            win.do_pool_action("delete");
                        }
                    }
                });
                dialog.present();
            }
        });

        // Volume callbacks
        let win = self.downgrade();
        imp.pool_details_view.set_on_add_volume(move || {
            if let Some(win) = win.upgrade() {
                win.show_create_volume_dialog();
            }
        });

        let win = self.downgrade();
        imp.pool_details_view.set_on_delete_volume(move |vol_name| {
            if let Some(win) = win.upgrade() {
                win.delete_volume(&vol_name);
            }
        });

        let win = self.downgrade();
        imp.pool_details_view.set_on_autostart(move |enabled| {
            if let Some(win) = win.upgrade() {
                win.set_pool_autostart(enabled);
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
                            win.imp().outer_stack.set_visible_child_name("empty");
                            win.imp().view_switcher_title.set_title("");
                            win.imp().view_switcher_title.set_subtitle("");
                            win.update_button_sensitivity(None);
                            win.stop_perf_sampling();
                            "VM deleted"
                        }
                        "console" => "Console launched",
                        _ => "Done",
                    };
                    win.show_toast(msg);
                    win.refresh_vm_list();

                    if matches!(action.as_str(), "start" | "force_stop" | "shutdown") {
                        if let Some(uuid) = win.imp().selected_uuid.borrow().clone() {
                            win.load_vm_details(&uuid);
                        }
                    }
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

        let mut to_remove: Vec<u32> = existing
            .iter()
            .filter(|(uuid, _)| !new_uuids.contains(*uuid))
            .map(|(_, (idx, _))| *idx)
            .collect();
        to_remove.sort_unstable_by(|a, b| b.cmp(a));
        for idx in to_remove {
            store.remove(idx);
        }

        for vm_info in vms {
            if let Some((_, obj)) = existing.get(&vm_info.uuid) {
                obj.update_from(vm_info);
            } else {
                store.append(&VmObject::new(vm_info));
            }
        }

        if let Some(ref uuid) = selected_uuid {
            let state = vms.iter().find(|v| v.uuid == *uuid).map(|v| v.state);

            let sidebar = self.imp().active_sidebar.borrow().clone();
            if sidebar == "vms" {
                self.update_button_sensitivity(state);
            }

            if let Some(vm_info) = vms.iter().find(|v| v.uuid == *uuid) {
                self.imp().view_switcher_title.set_subtitle(vm_info.state.label());

                if vm_info.state != backend::types::VmState::Running {
                    self.stop_perf_sampling();
                }
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
                let iface_targets = backend::domain_xml::extract_interface_targets(&xml);
                let disk_targets: Vec<String> = details.disks.iter().map(|d| d.target_dev.clone()).collect();
                let autostart = backend::domain::get_autostart(&uri, &uuid)?;
                let vms = backend::connection::list_all_vms(&uri)?;
                let vm_info = vms.into_iter().find(|v| v.uuid == uuid);
                Ok::<_, crate::error::AppError>((details, vm_info, autostart, disk_targets, iface_targets))
            }
        });

        glib::spawn_future_local(async move {
            let Ok(result) = rx.recv().await else { return };
            let Some(win) = win.upgrade() else { return };

            match result {
                Ok((details, vm_info, autostart, disk_targets, iface_targets)) => {
                    let state_label = vm_info
                        .as_ref()
                        .map(|v| v.state.label())
                        .unwrap_or("Unknown");
                    let domain_id = vm_info.as_ref().and_then(|v| v.id);

                    win.imp().details_view.update(&details, state_label, domain_id, autostart);
                    win.imp().outer_stack.set_visible_child_name("vm-content");

                    let uuid_for_snap = win.imp().selected_uuid.borrow().clone();
                    if let Some(uuid) = uuid_for_snap {
                        win.load_snapshots(&uuid);
                    }

                    *win.imp().disk_targets.borrow_mut() = disk_targets;
                    *win.imp().iface_targets.borrow_mut() = iface_targets;

                    let state = vm_info.map(|v| v.state);
                    win.update_button_sensitivity(state);

                    if state == Some(backend::types::VmState::Running) {
                        win.imp().perf_view.clear();
                        win.start_perf_sampling();
                    } else {
                        win.stop_perf_sampling();
                        win.imp().perf_view.clear();
                    }
                }
                Err(e) => {
                    win.show_toast(&format!("Failed to load details: {e}"));
                }
            }
        });
    }

    fn start_perf_sampling(&self) {
        if self.imp().perf_timer_id.borrow().is_some() {
            return;
        }

        *self.imp().last_perf_sample.borrow_mut() = None;

        let win = self.downgrade();
        let source_id = glib::timeout_add_local(std::time::Duration::from_secs(2), move || {
            let Some(win) = win.upgrade() else {
                return glib::ControlFlow::Break;
            };

            let uri = win.imp().connection_uri.borrow().clone();
            let uuid = win.imp().selected_uuid.borrow().clone();
            let Some(uuid) = uuid else {
                return glib::ControlFlow::Continue;
            };

            let disk_targets = win.imp().disk_targets.borrow().clone();
            let iface_targets = win.imp().iface_targets.borrow().clone();

            let rx = spawn_blocking(move || {
                backend::performance::collect_perf_sample(&uri, &uuid, &disk_targets, &iface_targets)
            });

            let win2 = win.downgrade();
            glib::spawn_future_local(async move {
                let Ok(result) = rx.recv().await else { return };
                let Some(win) = win2.upgrade() else { return };

                match result {
                    Ok(sample) => {
                        win.process_perf_sample(sample);
                    }
                    Err(e) => {
                        log::debug!("Perf sample failed: {e}");
                    }
                }
            });

            glib::ControlFlow::Continue
        });

        *self.imp().perf_timer_id.borrow_mut() = Some(source_id);
    }

    fn stop_perf_sampling(&self) {
        if let Some(source_id) = self.imp().perf_timer_id.borrow_mut().take() {
            source_id.remove();
        }
        *self.imp().last_perf_sample.borrow_mut() = None;
    }

    fn process_perf_sample(&self, sample: RawPerfSample) {
        use std::time::Instant;

        let now = Instant::now();
        let prev = self.imp().last_perf_sample.borrow_mut().take();

        if let Some((prev_time, prev_sample)) = prev {
            let wall_delta_ns = now.duration_since(prev_time).as_nanos() as f64;
            if wall_delta_ns > 0.0 {
                let cpu_delta = (sample.cpu_time_ns as f64) - (prev_sample.cpu_time_ns as f64);
                let cpu_percent = (cpu_delta / (wall_delta_ns * sample.nr_vcpus as f64)) * 100.0;
                let cpu_percent = cpu_percent.clamp(0.0, 100.0);

                let mem_total_kib = sample.memory_total_kib as f64;
                let mem_unused_kib = sample.memory_unused_kib as f64;
                let mem_used_kib = if mem_unused_kib > 0.0 {
                    mem_total_kib - mem_unused_kib
                } else {
                    mem_total_kib
                };
                let memory_used_percent = if mem_total_kib > 0.0 {
                    (mem_used_kib / mem_total_kib * 100.0).clamp(0.0, 100.0)
                } else {
                    0.0
                };

                let wall_delta_sec = wall_delta_ns / 1_000_000_000.0;

                let disk_rd_delta = (sample.disk_rd_bytes - prev_sample.disk_rd_bytes).max(0) as f64;
                let disk_wr_delta = (sample.disk_wr_bytes - prev_sample.disk_wr_bytes).max(0) as f64;
                let net_rx_delta = (sample.net_rx_bytes - prev_sample.net_rx_bytes).max(0) as f64;
                let net_tx_delta = (sample.net_tx_bytes - prev_sample.net_tx_bytes).max(0) as f64;

                let point = backend::types::PerfDataPoint {
                    cpu_percent,
                    memory_used_percent,
                    memory_used_mib: mem_used_kib / 1024.0,
                    memory_total_mib: mem_total_kib / 1024.0,
                    disk_read_bytes_sec: disk_rd_delta / wall_delta_sec,
                    disk_write_bytes_sec: disk_wr_delta / wall_delta_sec,
                    net_rx_bytes_sec: net_rx_delta / wall_delta_sec,
                    net_tx_bytes_sec: net_tx_delta / wall_delta_sec,
                };

                self.imp().perf_view.update(&point);
            }
        }

        *self.imp().last_perf_sample.borrow_mut() = Some((now, sample));
    }

    // --- Storage Pool methods ---

    fn refresh_pool_list(&self) {
        let uri = self.imp().connection_uri.borrow().clone();
        let win = self.downgrade();

        let rx = spawn_blocking(move || backend::storage::list_all_pools(&uri));

        glib::spawn_future_local(async move {
            let Ok(result) = rx.recv().await else { return };
            let Some(win) = win.upgrade() else { return };

            match result {
                Ok(pools) => {
                    win.update_pool_list(&pools);
                }
                Err(e) => {
                    log::error!("Failed to list pools: {e}");
                }
            }
        });
    }

    fn update_pool_list(&self, pools: &[backend::types::PoolInfo]) {
        let store = &self.imp().pool_list_store;

        let mut existing: std::collections::HashMap<String, (u32, PoolObject)> =
            std::collections::HashMap::new();
        for i in 0..store.n_items() {
            if let Some(obj) = store.item(i) {
                let pool = obj.downcast_ref::<PoolObject>().unwrap();
                existing.insert(pool.uuid(), (i, pool.clone()));
            }
        }

        let new_uuids: std::collections::HashSet<String> =
            pools.iter().map(|p| p.uuid.clone()).collect();

        let mut to_remove: Vec<u32> = existing
            .iter()
            .filter(|(uuid, _)| !new_uuids.contains(*uuid))
            .map(|(_, (idx, _))| *idx)
            .collect();
        to_remove.sort_unstable_by(|a, b| b.cmp(a));
        for idx in to_remove {
            store.remove(idx);
        }

        for pool_info in pools {
            if let Some((_, obj)) = existing.get(&pool_info.uuid) {
                obj.update_from(pool_info);
            } else {
                store.append(&PoolObject::new(pool_info));
            }
        }
    }

    fn load_pool_details(&self, uuid: &str) {
        let uri = self.imp().connection_uri.borrow().clone();
        let uuid = uuid.to_string();
        let win = self.downgrade();

        let rx = spawn_blocking({
            let uri = uri.clone();
            let uuid = uuid.clone();
            move || {
                let pools = backend::storage::list_all_pools(&uri)?;
                let pool_info = pools.into_iter().find(|p| p.uuid == uuid);
                let pool_xml = backend::storage::get_pool_xml(&uri, &uuid)?;
                let (pool_type, pool_path) = backend::storage::extract_pool_type_and_path(&pool_xml);
                let volumes = if pool_info.as_ref().map(|p| p.active).unwrap_or(false) {
                    backend::storage::list_pool_volumes(&uri, &uuid).unwrap_or_default()
                } else {
                    Vec::new()
                };
                Ok::<_, crate::error::AppError>((pool_info, volumes, pool_type, pool_path))
            }
        });

        glib::spawn_future_local(async move {
            let Ok(result) = rx.recv().await else { return };
            let Some(win) = win.upgrade() else { return };

            match result {
                Ok((Some(pool_info), volumes, pool_type, pool_path)) => {
                    win.imp().pool_details_view.update(&pool_info, &volumes, &pool_type, &pool_path);
                    win.imp().outer_stack.set_visible_child_name("pool-content");
                    win.imp().view_switcher_title.set_title(&pool_info.name);
                    win.imp().view_switcher_title.set_subtitle(pool_info.state.label());
                }
                Ok((None, _, _, _)) => {
                    win.show_toast("Pool not found");
                }
                Err(e) => {
                    win.show_toast(&format!("Failed to load pool: {e}"));
                }
            }
        });
    }

    fn do_pool_action(&self, action: &str) {
        let uuid = self.imp().selected_pool_uuid.borrow().clone();
        let uri = self.imp().connection_uri.borrow().clone();

        let Some(uuid) = uuid else { return };

        let win = self.downgrade();
        let action = action.to_string();

        let rx = spawn_blocking({
            let uuid = uuid.clone();
            let uri = uri.clone();
            let action = action.clone();
            move || match action.as_str() {
                "start" => backend::storage::start_pool(&uri, &uuid),
                "stop" => backend::storage::stop_pool(&uri, &uuid),
                "refresh" => backend::storage::refresh_pool(&uri, &uuid),
                "delete" => backend::storage::delete_pool(&uri, &uuid),
                _ => Ok(()),
            }
        });

        glib::spawn_future_local(async move {
            let Ok(result) = rx.recv().await else { return };
            let Some(win) = win.upgrade() else { return };

            match result {
                Ok(()) => {
                    let msg = match action.as_str() {
                        "start" => "Pool started",
                        "stop" => "Pool stopped",
                        "refresh" => "Pool refreshed",
                        "delete" => {
                            *win.imp().selected_pool_uuid.borrow_mut() = None;
                            win.imp().outer_stack.set_visible_child_name("pool-empty");
                            win.imp().view_switcher_title.set_title("");
                            win.imp().view_switcher_title.set_subtitle("");
                            "Pool deleted"
                        }
                        _ => "Done",
                    };
                    win.show_toast(msg);
                    win.refresh_pool_list();

                    if !matches!(action.as_str(), "delete") {
                        if let Some(uuid) = win.imp().selected_pool_uuid.borrow().clone() {
                            win.load_pool_details(&uuid);
                        }
                    }
                }
                Err(e) => {
                    win.show_toast(&format!("Error: {e}"));
                }
            }
        });
    }

    fn show_create_pool_dialog(&self) {
        let win = self.downgrade();
        crate::ui::create_pool_dialog::show_create_pool_dialog(self.upcast_ref(), move |name, pool_type, params| {
            let Some(win) = win.upgrade() else { return };
            let uri = win.imp().connection_uri.borrow().clone();

            let rx = spawn_blocking(move || {
                backend::storage::create_pool(&uri, &name, &pool_type, &params)
            });

            let win2 = win.downgrade();
            glib::spawn_future_local(async move {
                let Ok(result) = rx.recv().await else { return };
                let Some(win) = win2.upgrade() else { return };

                match result {
                    Ok(()) => {
                        win.show_toast("Pool created successfully");
                        win.refresh_pool_list();
                    }
                    Err(e) => {
                        win.show_toast(&format!("Failed to create pool: {e}"));
                    }
                }
            });
        });
    }

    fn show_create_volume_dialog(&self) {
        let win = self.downgrade();
        crate::ui::create_volume_dialog::show_create_volume_dialog(self.upcast_ref(), move |name, capacity_bytes, format| {
            let Some(win) = win.upgrade() else { return };
            let uri = win.imp().connection_uri.borrow().clone();
            let pool_uuid = win.imp().selected_pool_uuid.borrow().clone();
            let Some(pool_uuid) = pool_uuid else { return };

            let rx = spawn_blocking(move || {
                backend::storage::create_volume(&uri, &pool_uuid, &name, capacity_bytes, &format)
            });

            let win2 = win.downgrade();
            let pool_uuid2 = win.imp().selected_pool_uuid.borrow().clone();
            glib::spawn_future_local(async move {
                let Ok(result) = rx.recv().await else { return };
                let Some(win) = win2.upgrade() else { return };

                match result {
                    Ok(()) => {
                        win.show_toast("Volume created");
                        if let Some(uuid) = pool_uuid2 {
                            win.load_pool_details(&uuid);
                        }
                    }
                    Err(e) => {
                        win.show_toast(&format!("Failed to create volume: {e}"));
                    }
                }
            });
        });
    }

    fn set_pool_autostart(&self, enabled: bool) {
        let uri = self.imp().connection_uri.borrow().clone();
        let pool_uuid = self.imp().selected_pool_uuid.borrow().clone();
        let Some(pool_uuid) = pool_uuid else { return };

        let win = self.downgrade();

        let rx = spawn_blocking(move || {
            backend::storage::set_pool_autostart(&uri, &pool_uuid, enabled)
        });

        glib::spawn_future_local(async move {
            let Ok(result) = rx.recv().await else { return };
            let Some(win) = win.upgrade() else { return };

            match result {
                Ok(()) => {
                    let msg = if enabled { "Autostart enabled" } else { "Autostart disabled" };
                    win.show_toast(msg);
                }
                Err(e) => {
                    win.show_toast(&format!("Failed to set autostart: {e}"));
                }
            }
        });
    }

    fn delete_volume(&self, vol_name: &str) {
        let vol_name = vol_name.to_string();

        let dialog = adw::MessageDialog::new(
            Some(self),
            Some("Delete Volume?"),
            Some(&format!("This will permanently delete the volume \"{vol_name}\". This cannot be undone.")),
        );
        dialog.add_response("cancel", "Cancel");
        dialog.add_response("confirm", "Delete");
        dialog.set_response_appearance("confirm", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");

        let win = self.downgrade();
        dialog.connect_response(None, move |_, response| {
            if response != "confirm" {
                return;
            }
            let Some(win) = win.upgrade() else { return };

            let uri = win.imp().connection_uri.borrow().clone();
            let pool_uuid = win.imp().selected_pool_uuid.borrow().clone();
            let Some(pool_uuid) = pool_uuid else { return };
            let vol_name = vol_name.clone();

            let win2 = win.downgrade();
            let pool_uuid2 = pool_uuid.clone();

            let rx = spawn_blocking(move || {
                backend::storage::delete_volume(&uri, &pool_uuid, &vol_name)
            });

            glib::spawn_future_local(async move {
                let Ok(result) = rx.recv().await else { return };
                let Some(win) = win2.upgrade() else { return };

                match result {
                    Ok(()) => {
                        win.show_toast("Volume deleted");
                        win.load_pool_details(&pool_uuid2);
                    }
                    Err(e) => {
                        win.show_toast(&format!("Failed to delete volume: {e}"));
                    }
                }
            });
        });

        dialog.present();
    }

    // --- Network methods ---

    fn connect_network_action_buttons(&self) {
        let imp = self.imp();

        let win = self.downgrade();
        imp.network_details_view.btn_start.connect_clicked(move |_| {
            if let Some(win) = win.upgrade() {
                win.do_network_action("start");
            }
        });

        let win = self.downgrade();
        imp.network_details_view.btn_stop.connect_clicked(move |_| {
            if let Some(win) = win.upgrade() {
                win.do_network_action("stop");
            }
        });

        let win = self.downgrade();
        imp.network_details_view.btn_delete.connect_clicked(move |_| {
            if let Some(win) = win.upgrade() {
                let dialog = adw::MessageDialog::new(Some(&win), Some("Delete Network?"), Some("This will undefine the virtual network."));
                dialog.add_response("cancel", "Cancel");
                dialog.add_response("confirm", "Delete");
                dialog.set_response_appearance("confirm", adw::ResponseAppearance::Destructive);
                dialog.set_default_response(Some("cancel"));
                dialog.set_close_response("cancel");

                let win2 = win.downgrade();
                dialog.connect_response(None, move |_, response| {
                    if response == "confirm" {
                        if let Some(win) = win2.upgrade() {
                            win.do_network_action("delete");
                        }
                    }
                });
                dialog.present();
            }
        });

        let win = self.downgrade();
        imp.network_details_view.set_on_autostart(move |enabled| {
            if let Some(win) = win.upgrade() {
                win.set_network_autostart(enabled);
            }
        });
    }

    fn refresh_network_list(&self) {
        let uri = self.imp().connection_uri.borrow().clone();
        let win = self.downgrade();

        let rx = spawn_blocking(move || backend::network::list_all_networks(&uri));

        glib::spawn_future_local(async move {
            let Ok(result) = rx.recv().await else { return };
            let Some(win) = win.upgrade() else { return };

            match result {
                Ok(networks) => {
                    win.update_network_list(&networks);
                }
                Err(e) => {
                    log::error!("Failed to list networks: {e}");
                }
            }
        });
    }

    fn update_network_list(&self, networks: &[backend::types::VirtNetworkInfo]) {
        let store = &self.imp().network_list_store;

        let mut existing: std::collections::HashMap<String, (u32, NetworkObject)> =
            std::collections::HashMap::new();
        for i in 0..store.n_items() {
            if let Some(obj) = store.item(i) {
                let network = obj.downcast_ref::<NetworkObject>().unwrap();
                existing.insert(network.uuid(), (i, network.clone()));
            }
        }

        let new_uuids: std::collections::HashSet<String> =
            networks.iter().map(|n| n.uuid.clone()).collect();

        let mut to_remove: Vec<u32> = existing
            .iter()
            .filter(|(uuid, _)| !new_uuids.contains(*uuid))
            .map(|(_, (idx, _))| *idx)
            .collect();
        to_remove.sort_unstable_by(|a, b| b.cmp(a));
        for idx in to_remove {
            store.remove(idx);
        }

        for net_info in networks {
            if let Some((_, obj)) = existing.get(&net_info.uuid) {
                obj.update_from(net_info);
            } else {
                store.append(&NetworkObject::new(net_info));
            }
        }
    }

    fn load_network_details(&self, uuid: &str) {
        let uri = self.imp().connection_uri.borrow().clone();
        let uuid = uuid.to_string();
        let win = self.downgrade();

        let rx = spawn_blocking({
            let uri = uri.clone();
            let uuid = uuid.clone();
            move || {
                let networks = backend::network::list_all_networks(&uri)?;
                let net_info = networks.into_iter().find(|n| n.uuid == uuid);
                Ok::<_, crate::error::AppError>(net_info)
            }
        });

        glib::spawn_future_local(async move {
            let Ok(result) = rx.recv().await else { return };
            let Some(win) = win.upgrade() else { return };

            match result {
                Ok(Some(net_info)) => {
                    win.imp().network_details_view.update(&net_info);
                    win.imp().outer_stack.set_visible_child_name("network-content");
                    win.imp().view_switcher_title.set_title(&net_info.name);
                    win.imp().view_switcher_title.set_subtitle(net_info.state.label());
                }
                Ok(None) => {
                    win.show_toast("Network not found");
                }
                Err(e) => {
                    win.show_toast(&format!("Failed to load network: {e}"));
                }
            }
        });
    }

    fn do_network_action(&self, action: &str) {
        let uuid = self.imp().selected_network_uuid.borrow().clone();
        let uri = self.imp().connection_uri.borrow().clone();

        let Some(uuid) = uuid else { return };

        let win = self.downgrade();
        let action = action.to_string();

        let rx = spawn_blocking({
            let uuid = uuid.clone();
            let uri = uri.clone();
            let action = action.clone();
            move || match action.as_str() {
                "start" => backend::network::start_network(&uri, &uuid),
                "stop" => backend::network::stop_network(&uri, &uuid),
                "delete" => backend::network::delete_network(&uri, &uuid),
                _ => Ok(()),
            }
        });

        glib::spawn_future_local(async move {
            let Ok(result) = rx.recv().await else { return };
            let Some(win) = win.upgrade() else { return };

            match result {
                Ok(()) => {
                    let msg = match action.as_str() {
                        "start" => "Network started",
                        "stop" => "Network stopped",
                        "delete" => {
                            *win.imp().selected_network_uuid.borrow_mut() = None;
                            win.imp().outer_stack.set_visible_child_name("network-empty");
                            win.imp().view_switcher_title.set_title("");
                            win.imp().view_switcher_title.set_subtitle("");
                            "Network deleted"
                        }
                        _ => "Done",
                    };
                    win.show_toast(msg);
                    win.refresh_network_list();

                    if !matches!(action.as_str(), "delete") {
                        if let Some(uuid) = win.imp().selected_network_uuid.borrow().clone() {
                            win.load_network_details(&uuid);
                        }
                    }
                }
                Err(e) => {
                    win.show_toast(&format!("Error: {e}"));
                }
            }
        });
    }

    fn set_network_autostart(&self, enabled: bool) {
        let uri = self.imp().connection_uri.borrow().clone();
        let network_uuid = self.imp().selected_network_uuid.borrow().clone();
        let Some(network_uuid) = network_uuid else { return };

        let win = self.downgrade();

        let rx = spawn_blocking(move || {
            backend::network::set_network_autostart(&uri, &network_uuid, enabled)
        });

        glib::spawn_future_local(async move {
            let Ok(result) = rx.recv().await else { return };
            let Some(win) = win.upgrade() else { return };

            match result {
                Ok(()) => {
                    let msg = if enabled { "Autostart enabled" } else { "Autostart disabled" };
                    win.show_toast(msg);
                }
                Err(e) => {
                    win.show_toast(&format!("Failed to set autostart: {e}"));
                }
            }
        });
    }

    fn show_create_network_dialog(&self) {
        let win = self.downgrade();
        crate::ui::create_network_dialog::show_create_network_dialog(self.upcast_ref(), move |params| {
            let Some(win) = win.upgrade() else { return };
            let uri = win.imp().connection_uri.borrow().clone();

            let rx = spawn_blocking(move || {
                backend::network::create_network(&uri, &params)
            });

            let win2 = win.downgrade();
            glib::spawn_future_local(async move {
                let Ok(result) = rx.recv().await else { return };
                let Some(win) = win2.upgrade() else { return };

                match result {
                    Ok(()) => {
                        win.show_toast("Network created successfully");
                        win.refresh_network_list();
                    }
                    Err(e) => {
                        win.show_toast(&format!("Failed to create network: {e}"));
                    }
                }
            });
        });
    }

    // --- VM dialogs ---

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

    // --- Snapshot methods ---

    fn connect_snapshot_callbacks(&self) {
        let win = self.downgrade();
        self.imp().snapshot_view.set_on_create(move || {
            if let Some(win) = win.upgrade() {
                win.show_create_snapshot_dialog();
            }
        });

        let win = self.downgrade();
        self.imp().snapshot_view.set_on_revert(move |snap_name| {
            if let Some(win) = win.upgrade() {
                win.confirm_and_revert_snapshot(&snap_name);
            }
        });

        let win = self.downgrade();
        self.imp().snapshot_view.set_on_delete(move |snap_name| {
            if let Some(win) = win.upgrade() {
                win.confirm_and_delete_snapshot(&snap_name);
            }
        });
    }

    fn load_snapshots(&self, uuid: &str) {
        let uri = self.imp().connection_uri.borrow().clone();
        let uuid = uuid.to_string();
        let win = self.downgrade();

        let rx = spawn_blocking(move || backend::snapshot::list_snapshots(&uri, &uuid));

        glib::spawn_future_local(async move {
            let Ok(result) = rx.recv().await else { return };
            let Some(win) = win.upgrade() else { return };

            match result {
                Ok(snapshots) => {
                    win.imp().snapshot_view.update(&snapshots);
                }
                Err(e) => {
                    log::error!("Failed to load snapshots: {e}");
                    win.imp().snapshot_view.update(&[]);
                }
            }
        });
    }

    fn show_create_snapshot_dialog(&self) {
        let win = self.downgrade();
        crate::ui::create_snapshot_dialog::show_create_snapshot_dialog(
            self.upcast_ref(),
            move |name, description| {
                let Some(win) = win.upgrade() else { return };
                let uri = win.imp().connection_uri.borrow().clone();
                let uuid = win.imp().selected_uuid.borrow().clone();
                let Some(uuid) = uuid else { return };

                let params = backend::types::CreateSnapshotParams { name, description };

                let rx = spawn_blocking({
                    let uuid = uuid.clone();
                    move || backend::snapshot::create_snapshot(&uri, &uuid, &params)
                });

                let win2 = win.downgrade();
                glib::spawn_future_local(async move {
                    let Ok(result) = rx.recv().await else { return };
                    let Some(win) = win2.upgrade() else { return };

                    match result {
                        Ok(()) => {
                            win.show_toast("Snapshot created");
                            win.load_snapshots(&uuid);
                        }
                        Err(e) => {
                            win.show_toast(&format!("Failed to create snapshot: {e}"));
                        }
                    }
                });
            },
        );
    }

    fn confirm_and_revert_snapshot(&self, snap_name: &str) {
        let snap_name = snap_name.to_string();
        let dialog = adw::MessageDialog::new(
            Some(self),
            Some("Revert to Snapshot?"),
            Some(&format!(
                "This will revert the VM to snapshot \"{snap_name}\". The current state will be lost."
            )),
        );
        dialog.add_response("cancel", "Cancel");
        dialog.add_response("confirm", "Revert");
        dialog.set_response_appearance("confirm", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");

        let win = self.downgrade();
        dialog.connect_response(None, move |_, response| {
            if response != "confirm" {
                return;
            }
            let Some(win) = win.upgrade() else { return };

            let uri = win.imp().connection_uri.borrow().clone();
            let uuid = win.imp().selected_uuid.borrow().clone();
            let Some(uuid) = uuid else { return };
            let snap_name = snap_name.clone();

            let win2 = win.downgrade();
            let uuid2 = uuid.clone();

            let rx = spawn_blocking(move || {
                backend::snapshot::revert_snapshot(&uri, &uuid, &snap_name)
            });

            glib::spawn_future_local(async move {
                let Ok(result) = rx.recv().await else { return };
                let Some(win) = win2.upgrade() else { return };

                match result {
                    Ok(()) => {
                        win.show_toast("Reverted to snapshot");
                        win.refresh_vm_list();
                        win.load_vm_details(&uuid2);
                    }
                    Err(e) => {
                        win.show_toast(&format!("Failed to revert: {e}"));
                    }
                }
            });
        });

        dialog.present();
    }

    fn confirm_and_delete_snapshot(&self, snap_name: &str) {
        let snap_name = snap_name.to_string();
        let dialog = adw::MessageDialog::new(
            Some(self),
            Some("Delete Snapshot?"),
            Some(&format!(
                "This will permanently delete the snapshot \"{snap_name}\". This cannot be undone."
            )),
        );
        dialog.add_response("cancel", "Cancel");
        dialog.add_response("confirm", "Delete");
        dialog.set_response_appearance("confirm", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");

        let win = self.downgrade();
        dialog.connect_response(None, move |_, response| {
            if response != "confirm" {
                return;
            }
            let Some(win) = win.upgrade() else { return };

            let uri = win.imp().connection_uri.borrow().clone();
            let uuid = win.imp().selected_uuid.borrow().clone();
            let Some(uuid) = uuid else { return };
            let snap_name = snap_name.clone();

            let win2 = win.downgrade();
            let uuid2 = uuid.clone();

            let rx = spawn_blocking(move || {
                backend::snapshot::delete_snapshot(&uri, &uuid, &snap_name)
            });

            glib::spawn_future_local(async move {
                let Ok(result) = rx.recv().await else { return };
                let Some(win) = win2.upgrade() else { return };

                match result {
                    Ok(()) => {
                        win.show_toast("Snapshot deleted");
                        win.load_snapshots(&uuid2);
                    }
                    Err(e) => {
                        win.show_toast(&format!("Failed to delete snapshot: {e}"));
                    }
                }
            });
        });

        dialog.present();
    }

    fn handle_config_action(
        uri: &str,
        uuid: &str,
        action: backend::types::ConfigAction,
    ) -> Result<(), crate::error::AppError> {
        use backend::types::ConfigAction;

        match action {
            ConfigAction::ApplyGeneral(changes) => {
                backend::domain::set_autostart(uri, uuid, changes.autostart)?;

                let xml = backend::domain::get_domain_xml(uri, uuid)?;

                let xml = if changes.vcpus > 0 && changes.memory_mib > 0 {
                    backend::domain_xml::modify_domain_xml(&xml, changes.vcpus, changes.memory_mib)?
                } else {
                    xml
                };

                let xml = backend::domain_xml::modify_cpu_model(
                    &xml,
                    changes.cpu_mode,
                    changes.cpu_model.as_deref(),
                )?;

                let xml = backend::domain_xml::modify_boot_order(&xml, &changes.boot_order)?;

                let xml = backend::domain_xml::modify_firmware(&xml, changes.firmware)?;

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
            ConfigAction::ModifyGraphics(gtype) => {
                let xml = backend::domain::get_domain_xml(uri, uuid)?;
                let xml = backend::domain_xml::modify_graphics(&xml, gtype)?;
                backend::domain::update_domain_xml(uri, &xml)?;
                Ok(())
            }
            ConfigAction::ModifyVideo(vmodel) => {
                let xml = backend::domain::get_domain_xml(uri, uuid)?;
                let xml = backend::domain_xml::modify_video(&xml, vmodel)?;
                backend::domain::update_domain_xml(uri, &xml)?;
                Ok(())
            }
            ConfigAction::ModifySound(smodel) => {
                let xml = backend::domain::get_domain_xml(uri, uuid)?;
                let xml = backend::domain_xml::modify_sound(&xml, smodel)?;
                backend::domain::update_domain_xml(uri, &xml)?;
                Ok(())
            }
        }
    }
}
