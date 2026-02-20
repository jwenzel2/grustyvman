/// grustyvman-viewer — embedded SPICE console for grustyvman.
///
/// Usage:
///   grustyvman-viewer \
///     --host HOST --port PORT \
///     --uri  LIBVIRT_URI  --uuid VM_UUID \
///     [--password PASS] [--title TITLE]
///
/// All GTK3 / spice-client-gtk work is done in spice_helpers.c; this file
/// handles argument parsing, VM control via the virt crate, and wiring the
/// Rust action callback into the C toolbar.

use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};

use virt::connect::Connect;
use virt::domain::Domain;

// ---------------------------------------------------------------------------
// FFI: C helpers compiled from spice_helpers.c
// ---------------------------------------------------------------------------

/// Opaque handle returned by grv_viewer_build().
type GrvViewer = c_void;

/// Action callback type — must match `GrvActionFn` typedef in spice_helpers.c.
type GrvActionFn = unsafe extern "C" fn(c_int, *mut c_void);

extern "C" {
    fn grv_session_create(
        host: *const c_char,
        port: *const c_char,
        password: *const c_char,
    ) -> *mut c_void; // SpiceSession*

    fn grv_session_connect(session: *mut c_void);

    fn grv_viewer_build(
        title: *const c_char,
        session: *mut c_void,
        action_fn: Option<GrvActionFn>,
        action_data: *mut c_void,
    ) -> *mut GrvViewer;

    fn grv_viewer_show(viewer: *mut GrvViewer);
}

// ---------------------------------------------------------------------------
// FFI: GTK3 main loop (transitively linked via spice-client-gtk-3.0)
// ---------------------------------------------------------------------------

extern "C" {
    fn gtk_init(argc: *mut c_int, argv: *mut *mut *mut c_char);
    fn gtk_main();
}

// ---------------------------------------------------------------------------
// VM control
// ---------------------------------------------------------------------------

/// Action IDs — must match the `#define GRV_ACTION_*` values in spice_helpers.c.
const ACTION_PAUSE: i32        = 0;
const ACTION_RESUME: i32       = 1;
const ACTION_SHUTDOWN: i32     = 2;
const ACTION_REBOOT: i32       = 3;
const ACTION_FORCE_STOP: i32   = 4;
const ACTION_FORCE_REBOOT: i32 = 5;

struct VmControl {
    uri: String,
    uuid: String,
}

impl VmControl {
    fn with_domain<T, F>(&self, f: F) -> Result<(), String>
    where
        F: FnOnce(&Domain) -> Result<T, virt::error::Error>,
    {
        let conn = Connect::open(Some(&self.uri))
            .map_err(|e| format!("libvirt connect: {e}"))?;
        let domain = Domain::lookup_by_uuid_string(&conn, &self.uuid)
            .map_err(|e| format!("domain lookup: {e}"))?;
        f(&domain).map(|_| ()).map_err(|e| format!("{e}"))
    }

    fn pause(&self)        -> Result<(), String> { self.with_domain(|d| d.suspend()) }
    fn resume(&self)       -> Result<(), String> { self.with_domain(|d| d.resume()) }
    fn shutdown(&self)     -> Result<(), String> { self.with_domain(|d| d.shutdown()) }
    fn reboot(&self)       -> Result<(), String> { self.with_domain(|d| d.reboot(0)) }
    fn force_stop(&self)   -> Result<(), String> { self.with_domain(|d| d.destroy()) }

    fn force_reboot(&self) -> Result<(), String> {
        // virDomainReset would be ideal here, but destroy+create is universally safe.
        self.with_domain(|d| d.destroy())?;
        std::thread::sleep(std::time::Duration::from_millis(400));
        self.with_domain(|d| d.create())
    }
}

/// GTK calls this from the main thread when a toolbar button is activated.
/// We spawn a thread so libvirt I/O never stalls the GTK event loop.
unsafe extern "C" fn vm_action_cb(action: c_int, user_data: *mut c_void) {
    let vm = &*(user_data as *const VmControl);
    // Clone connection info for the worker thread.
    let uri  = vm.uri.clone();
    let uuid = vm.uuid.clone();
    std::thread::spawn(move || {
        let vm = VmControl { uri, uuid };
        let result = match action {
            x if x == ACTION_PAUSE        => vm.pause(),
            x if x == ACTION_RESUME       => vm.resume(),
            x if x == ACTION_SHUTDOWN     => vm.shutdown(),
            x if x == ACTION_REBOOT       => vm.reboot(),
            x if x == ACTION_FORCE_STOP   => vm.force_stop(),
            x if x == ACTION_FORCE_REBOOT => vm.force_reboot(),
            other => {
                eprintln!("grustyvman-viewer: unknown action id {other}");
                return;
            }
        };
        if let Err(e) = result {
            eprintln!("grustyvman-viewer: VM action failed: {e}");
        }
    });
}

// ---------------------------------------------------------------------------
// CLI argument parsing
// ---------------------------------------------------------------------------

struct Args {
    host:     String,
    port:     String,
    password: String,
    title:    String,
    uri:      String,
    uuid:     String,
}

impl Args {
    fn parse() -> Self {
        let raw: Vec<String> = std::env::args().collect();
        let mut host     = "127.0.0.1".to_string();
        let mut port     = "5900".to_string();
        let mut password = String::new();
        let mut title    = "VM Console — SPICE".to_string();
        let mut uri      = String::new();
        let mut uuid     = String::new();

        let mut i = 1;
        while i < raw.len() {
            match raw[i].as_str() {
                "--host"     => { i += 1; if i < raw.len() { host     = raw[i].clone(); } }
                "--port"     => { i += 1; if i < raw.len() { port     = raw[i].clone(); } }
                "--password" => { i += 1; if i < raw.len() { password = raw[i].clone(); } }
                "--title"    => { i += 1; if i < raw.len() { title    = raw[i].clone(); } }
                "--uri"      => { i += 1; if i < raw.len() { uri      = raw[i].clone(); } }
                "--uuid"     => { i += 1; if i < raw.len() { uuid     = raw[i].clone(); } }
                other        => eprintln!("grustyvman-viewer: unknown argument: {other}"),
            }
            i += 1;
        }

        Args { host, port, password, title, uri, uuid }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    let args = Args::parse();

    let host_c     = CString::new(args.host.as_str()).expect("host contains NUL");
    let port_c     = CString::new(args.port.as_str()).expect("port contains NUL");
    let password_c = CString::new(args.password.as_str()).expect("password contains NUL");
    let title_c    = CString::new(args.title.as_str()).expect("title contains NUL");

    // Heap-allocate VmControl so it lives for the duration of the process and
    // can be handed to C as a stable pointer.
    let vm = Box::new(VmControl { uri: args.uri, uuid: args.uuid });
    let vm_ptr = Box::into_raw(vm);

    unsafe {
        gtk_init(std::ptr::null_mut(), std::ptr::null_mut());

        // Create the SPICE session (not yet connected).
        let session = grv_session_create(
            host_c.as_ptr(), port_c.as_ptr(), password_c.as_ptr(),
        );
        if session.is_null() {
            eprintln!("grustyvman-viewer: failed to create SPICE session");
            std::process::exit(1);
        }

        // Build the window + toolbar + SpiceDisplay.
        let viewer = grv_viewer_build(
            title_c.as_ptr(),
            session,
            Some(vm_action_cb),
            vm_ptr as *mut c_void,
        );
        if viewer.is_null() {
            eprintln!("grustyvman-viewer: failed to build viewer");
            std::process::exit(1);
        }

        grv_viewer_show(viewer);

        // Begin async SPICE connection (driven by GTK's GLib main loop).
        grv_session_connect(session);

        gtk_main();

        // gtk_main() returns when the window is closed.
        // Reclaim VmControl to avoid the leak warning.
        drop(Box::from_raw(vm_ptr));
    }
}
