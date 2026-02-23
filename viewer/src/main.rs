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
use std::sync::atomic::{AtomicUsize, Ordering};

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

    /// Switch the viewer to the powered-off page.  Must be called on the GTK
    /// main thread (use via g_idle_add from background threads).
    fn grv_viewer_set_powered_off(viewer: *mut GrvViewer);

    /// Show "Restoring Snapshot…" on the status page while waiting for SPICE
    /// to come back after a revert.  Must be called on the GTK main thread.
    fn grv_viewer_set_reverting(viewer: *mut GrvViewer);

    /// Reconfigure session host/port/password and reconnect. Must be called on
    /// the GTK main thread.
    fn grv_viewer_reconnect(
        viewer: *mut GrvViewer,
        host: *const c_char,
        port: *const c_char,
        password: *const c_char,
    );

    /// Show a "Creating snapshot…" progress dialog. Must be called on the GTK
    /// main thread. Returns the GtkWidget* (cast to *mut c_void) for later
    /// cleanup via grv_snapshot_end.
    fn grv_snapshot_begin(viewer: *mut GrvViewer) -> *mut c_void;

    /// Close the progress dialog and show an error if `success == 0`. Must be
    /// called on the GTK main thread (use via g_idle_add from worker threads).
    fn grv_snapshot_end(
        dialog: *mut c_void,
        viewer: *mut GrvViewer,
        success: c_int,
        message: *const c_char,
    );
}

// ---------------------------------------------------------------------------
// GLib idle-add for cross-thread GTK calls
// ---------------------------------------------------------------------------

extern "C" {
    /// Schedule `func(data)` to run on the GLib main loop thread.
    /// Returns the GSource ID (we don't need it so we ignore it).
    fn g_idle_add(
        func: unsafe extern "C" fn(*mut c_void) -> c_int,
        data: *mut c_void,
    ) -> u32;
}

/// GLib idle callback: switches the viewer to the powered-off screen.
/// The `data` pointer is the `GrvViewer *` cast to `*mut c_void`.
unsafe extern "C" fn idle_set_powered_off(data: *mut c_void) -> c_int {
    grv_viewer_set_powered_off(data as *mut GrvViewer);
    0 // G_SOURCE_REMOVE — run only once
}

/// GLib idle callback: switches the viewer to the "Restoring Snapshot…" screen.
unsafe extern "C" fn idle_set_reverting(data: *mut c_void) -> c_int {
    grv_viewer_set_reverting(data as *mut GrvViewer);
    0 // G_SOURCE_REMOVE
}

/// Integer-encoded viewer pointer for safe cross-thread transfer.
/// Raw pointers are not Send; storing the address as `usize` sidesteps that
/// while keeping the semantics identical (single-writer, GTK-main-thread-only
/// receiver via g_idle_add).
struct ViewerHandle(usize);

struct ActionContext {
    vm: VmControl,
    viewer_addr: AtomicUsize,
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
const ACTION_POWER_ON: i32      = 0;
const ACTION_PAUSE: i32         = 1;
const ACTION_RESUME: i32        = 2;
const ACTION_SHUTDOWN: i32      = 3;
const ACTION_REBOOT: i32        = 4;
const ACTION_FORCE_STOP: i32    = 5;
const ACTION_FORCE_REBOOT: i32  = 6;
const ACTION_SNAPSHOT: i32      = 7;
/// Fired by C when the SPICE main channel closes unexpectedly.
const ACTION_CHANNEL_CLOSED: i32 = 8;

struct VmControl {
    uri: String,
    uuid: String,
}

struct SpiceEndpoint {
    host: String,
    port: String,
    password: String,
}

impl VmControl {
    fn with_domain<T, F>(&self, f: F) -> Result<(), String>
    where
        F: FnOnce(&Domain) -> Result<T, virt::error::Error>,
    {
        let mut conn = Connect::open(Some(&self.uri))
            .map_err(|e| format!("libvirt connect: {e}"))?;
        let result = Domain::lookup_by_uuid_string(&conn, &self.uuid)
            .map_err(|e| format!("domain lookup: {e}"))
            .and_then(|domain| f(&domain).map(|_| ()).map_err(|e| format!("{e}")));
        let _ = conn.close();
        result
    }

    fn start(&self)        -> Result<(), String> { self.with_domain(|d| d.create()) }
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

    fn snapshot(&self) -> Result<(), String> {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let name = format!("snap-{ts}");
        let xml = format!("<domainsnapshot><name>{name}</name></domainsnapshot>");
        self.with_domain(|d| {
            virt::domain_snapshot::DomainSnapshot::create_xml(d, &xml, 0).map(|_| ())
        })
    }

    /// Returns `true` when the VM has an active SPICE server (running or
    /// paused).  Returns `true` on transient libvirt errors to avoid spurious
    /// "powered off" flashes.  Returns `false` when the domain is shut off,
    /// crashed, or cannot be found.
    fn is_active(&self) -> bool {
        let mut conn = match Connect::open(Some(&self.uri)) {
            Ok(c) => c,
            Err(_) => return true, // treat connection error as "still up"
        };
        let result = match Domain::lookup_by_uuid_string(&conn, &self.uuid) {
            Ok(domain) => match domain.get_state() {
                // VIR_DOMAIN_RUNNING=1, VIR_DOMAIN_BLOCKED=2,
                // VIR_DOMAIN_PAUSED=3, VIR_DOMAIN_SHUTDOWN=4 (still has SPICE)
                Ok((state, _)) => matches!(state, 1 | 2 | 3 | 4),
                Err(_) => true,
            },
            Err(_) => false, // domain gone → treat as shutoff
        };
        let _ = conn.close();
        result
    }
}

fn extract_attr(tag: &str, key: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let needle = format!("{key}={quote}");
        if let Some(start) = tag.find(&needle) {
            let rest = &tag[start + needle.len()..];
            if let Some(end) = rest.find(quote) {
                return Some(rest[..end].to_string());
            }
        }
    }
    None
}

fn parse_spice_endpoint(xml: &str) -> Option<SpiceEndpoint> {
    let mut search_from = 0usize;

    while let Some(rel) = xml[search_from..].find("<graphics") {
        let start = search_from + rel;
        let after = &xml[start..];
        let close = after.find('>')?;
        let open_tag = &after[..=close];

        if extract_attr(open_tag, "type").as_deref() != Some("spice") {
            search_from = start + close + 1;
            continue;
        }

        let port = extract_attr(open_tag, "port")
            .filter(|p| !p.is_empty() && p != "-1")?;
        let mut host = extract_attr(open_tag, "listen");
        if matches!(host.as_deref(), Some("") | Some("0.0.0.0")) {
            host = None;
        }

        let password = extract_attr(open_tag, "passwd")
            .or_else(|| extract_attr(open_tag, "password"))
            .unwrap_or_default();

        if host.is_none() && !open_tag.trim_end().ends_with("/>") {
            if let Some(end_rel) = after.find("</graphics>") {
                let body = &after[close + 1..end_rel];
                if let Some(listen_rel) = body.find("<listen") {
                    let listen_after = &body[listen_rel..];
                    if let Some(listen_close) = listen_after.find('>') {
                        let listen_tag = &listen_after[..=listen_close];
                        host = extract_attr(listen_tag, "address")
                            .or_else(|| extract_attr(listen_tag, "host"));
                        if matches!(host.as_deref(), Some("") | Some("0.0.0.0")) {
                            host = None;
                        }
                    }
                }
            }
        }

        return Some(SpiceEndpoint {
            host: host.unwrap_or_else(|| "127.0.0.1".to_string()),
            port,
            password,
        });
    }

    None
}

/// Heap-allocated context passed from the snapshot worker thread to the GTK
/// main thread via g_idle_add.
struct SnapshotEndCtx {
    dialog_addr: usize,
    viewer_addr: usize,
    success: c_int,
    /// Owned error string (None on success).
    message: Option<CString>,
}

unsafe extern "C" fn idle_snapshot_end(data: *mut c_void) -> c_int {
    let ctx = Box::from_raw(data as *mut SnapshotEndCtx);
    grv_snapshot_end(
        ctx.dialog_addr as *mut c_void,
        ctx.viewer_addr as *mut GrvViewer,
        ctx.success,
        ctx.message.as_ref().map_or(std::ptr::null(), |m| m.as_ptr()),
    );
    0 // G_SOURCE_REMOVE
}

#[repr(C)]
struct ReconnectRequest {
    viewer: *mut GrvViewer,
    host: CString,
    port: CString,
    password: CString,
}

unsafe extern "C" fn idle_reconnect(data: *mut c_void) -> c_int {
    let req = Box::from_raw(data as *mut ReconnectRequest);
    grv_viewer_reconnect(
        req.viewer,
        req.host.as_ptr(),
        req.port.as_ptr(),
        req.password.as_ptr(),
    );
    0 // G_SOURCE_REMOVE
}

fn schedule_reconnect(viewer_addr: usize, endpoint: SpiceEndpoint) {
    if viewer_addr == 0 {
        return;
    }
    let Ok(host) = CString::new(endpoint.host) else { return };
    let Ok(port) = CString::new(endpoint.port) else { return };
    let Ok(password) = CString::new(endpoint.password) else { return };

    let req = Box::new(ReconnectRequest {
        viewer: viewer_addr as *mut GrvViewer,
        host,
        port,
        password,
    });

    unsafe {
        g_idle_add(idle_reconnect, Box::into_raw(req) as *mut c_void);
    }
}

fn wait_for_spice_endpoint(vm: &VmControl, timeout_secs: u64) -> Option<SpiceEndpoint> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    // Keep a single persistent connection for all retries.  Opening and closing a
    // new libvirt connection on every iteration (up to 60×) registers GLib I/O
    // event sources that are cleaned up asynchronously by the GTK main loop; rapid
    // accumulation can exhaust the process FD limit before cleanup catches up.
    let mut conn: Option<Connect> = Connect::open(Some(&vm.uri)).ok();
    loop {
        if conn.is_none() {
            conn = Connect::open(Some(&vm.uri)).ok();
        }
        let mut lookup_failed = false;
        if let Some(c) = conn.as_mut() {
            match Domain::lookup_by_uuid_string(c, &vm.uuid) {
                Ok(domain) => {
                    let ep = domain.get_state().ok()
                        .filter(|(s, _)| matches!(s, 1 | 2 | 3 | 4))
                        .and_then(|_| domain.get_xml_desc(1).ok())
                        .and_then(|xml| parse_spice_endpoint(&xml));
                    if ep.is_some() {
                        if let Some(mut c) = conn { let _ = c.close(); }
                        return ep;
                    }
                }
                Err(_) => lookup_failed = true, // connection stale; reopen next iteration
            }
        }
        if lookup_failed { conn = None; }
        if std::time::Instant::now() >= deadline {
            if let Some(mut c) = conn { let _ = c.close(); }
            return None;
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

/// GTK calls this from the main thread when a toolbar button is activated.
/// We spawn a thread so libvirt I/O never stalls the GTK event loop.
unsafe extern "C" fn vm_action_cb(action: c_int, user_data: *mut c_void) {
    let ctx = &*(user_data as *const ActionContext);
    // Clone connection info for the worker thread.
    let uri = ctx.vm.uri.clone();
    let uuid = ctx.vm.uuid.clone();
    let viewer_addr = ctx.viewer_addr.load(Ordering::Relaxed);

    // Snapshot gets special treatment: show a progress dialog while the
    // (potentially slow) libvirt call runs, then show success or error.
    if action == ACTION_SNAPSHOT {
        let dialog_ptr = if viewer_addr != 0 {
            grv_snapshot_begin(viewer_addr as *mut GrvViewer)
        } else {
            std::ptr::null_mut()
        };
        let dialog_addr = dialog_ptr as usize;

        std::thread::spawn(move || {
            let vm = VmControl { uri, uuid: uuid.clone() };
            let result = vm.snapshot();
            let success = result.is_ok();

            // Write a tiny signal file so the main grustyvman app can detect
            // the new snapshot via GFileMonitor and refresh its snapshot list.
            if success {
                let signal_path = format!("/tmp/grustyvman-snap-{uuid}");
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let _ = std::fs::write(&signal_path, ts.to_string());
            }

            let end_ctx = Box::new(SnapshotEndCtx {
                dialog_addr,
                viewer_addr,
                success: if success { 1 } else { 0 },
                message: result.err().and_then(|e| CString::new(e).ok()),
            });
            g_idle_add(idle_snapshot_end, Box::into_raw(end_ctx) as *mut c_void);
        });
        return;
    }

    // SPICE main channel closed: check if the VM is still active and reconnect
    // if so (e.g. after a snapshot revert).  If the VM is off, show the
    // powered-off page.
    if action == ACTION_CHANNEL_CLOSED {
        std::thread::spawn(move || {
            // Brief delay so the VM state settles after the SPICE disconnect.
            std::thread::sleep(std::time::Duration::from_millis(500));
            let vm = VmControl { uri, uuid };
            if vm.is_active() {
                eprintln!("grustyvman-viewer: SPICE closed but VM is active — reconnecting");
                if let Some(endpoint) = wait_for_spice_endpoint(&vm, 20) {
                    schedule_reconnect(viewer_addr, endpoint);
                    return;
                }
                eprintln!("grustyvman-viewer: timed out waiting for SPICE after disconnect");
            }
            // VM is off (or SPICE never came back) — show powered-off page.
            if viewer_addr != 0 {
                unsafe {
                    g_idle_add(idle_set_powered_off, viewer_addr as *mut c_void);
                }
            }
        });
        return;
    }

    std::thread::spawn(move || {
        let vm = VmControl { uri, uuid };
        let is_power_on = action == ACTION_POWER_ON;
        let result = match action {
            x if x == ACTION_POWER_ON     => vm.start(),
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

        if is_power_on {
            if let Some(endpoint) = wait_for_spice_endpoint(&vm, 60) {
                eprintln!(
                    "grustyvman-viewer: reconnecting to SPICE {}:{}",
                    endpoint.host, endpoint.port
                );
                schedule_reconnect(viewer_addr, endpoint);
            } else {
                eprintln!("grustyvman-viewer: timed out waiting for SPICE endpoint after power on");
            }
        }

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
    if let Ok(exe) = std::env::current_exe() {
        eprintln!(
            "grustyvman-viewer: startup reconnect-build {} ({})",
            env!("CARGO_PKG_VERSION"),
            exe.display()
        );
    }

    let host_c     = CString::new(args.host.as_str()).expect("host contains NUL");
    let port_c     = CString::new(args.port.as_str()).expect("port contains NUL");
    let password_c = CString::new(args.password.as_str()).expect("password contains NUL");
    let title_c    = CString::new(args.title.as_str()).expect("title contains NUL");

    // Keep copies of uri/uuid for the libvirt polling thread.
    let poll_uri  = args.uri.clone();
    let poll_uuid = args.uuid.clone();

    // Heap-allocate VmControl so it lives for the duration of the process and
    // can be handed to C as a stable pointer.
    let action_ctx = Box::new(ActionContext {
        vm: VmControl {
            uri: args.uri,
            uuid: args.uuid,
        },
        viewer_addr: AtomicUsize::new(0),
    });
    let action_ctx_ptr = Box::into_raw(action_ctx);

    // ── GTK / SPICE setup (unsafe) ─────────────────────────────────────────
    let viewer_handle = unsafe {
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
            action_ctx_ptr as *mut c_void,
        );
        if viewer.is_null() {
            eprintln!("grustyvman-viewer: failed to build viewer");
            std::process::exit(1);
        }

        (*action_ctx_ptr)
            .viewer_addr
            .store(viewer as usize, Ordering::Relaxed);

        grv_viewer_show(viewer);

        // Begin async SPICE connection (driven by GTK's GLib main loop).
        grv_session_connect(session);

        ViewerHandle(viewer as usize)
    };

    let poll_viewer_addr = viewer_handle.0;

    // ── Libvirt VM-state polling thread ─────────────────────────────────────
    // Runs forever and tracks transitions in *both* directions:
    //
    //   active → inactive : show the powered-off page (handles ACPI shutdown
    //                        where QEMU keeps SPICE alive and CHANNEL_CLOSED
    //                        is never emitted)
    //
    //   inactive → active : reconnect the display (handles snapshot reverts,
    //                        external power-on, or any other case where the VM
    //                        comes back while the viewer is on the powered-off
    //                        page)
    std::thread::spawn(move || {
        let poll_vm = VmControl { uri: poll_uri, uuid: poll_uuid };

        // Keep a single persistent libvirt connection for the life of this
        // thread.  Re-opening and closing a connection every 2 s leaks file
        // descriptors (virConnectOpen creates sockets + GLib event sources that
        // aren't freed until the connection's reference count reaches zero, and
        // the virt crate's Connect has no Drop impl).  A persistent connection
        // is also faster and is the idiomatic libvirt usage pattern.
        let mut conn: Option<Connect> = Connect::open(Some(&poll_vm.uri))
            .map_err(|e| eprintln!("grustyvman-viewer: initial poll conn failed: {e}"))
            .ok();

        // Helper: check whether the VM is active using the persistent connection.
        // Returns true on any transient error so we don't spuriously show
        // "powered off" when libvirtd hiccups.
        let is_active_conn = |conn: &mut Option<Connect>| -> bool {
            // Reopen the connection if it was previously lost.
            if conn.is_none() {
                *conn = Connect::open(Some(&poll_vm.uri)).ok();
            }
            match conn.as_mut() {
                None => true, // connection error → assume still up
                Some(c) => match Domain::lookup_by_uuid_string(c, &poll_vm.uuid) {
                    Ok(domain) => match domain.get_state() {
                        Ok((state, _)) => matches!(state, 1 | 2 | 3 | 4),
                        Err(_) => true,
                    },
                    Err(e) => {
                        // Could be the domain is gone OR the connection is stale.
                        // Drop the connection so we reopen next poll.
                        eprintln!("grustyvman-viewer: domain lookup failed: {e}");
                        *conn = None;
                        false
                    }
                },
            }
        };

        // Seed the initial state so we don't fire spuriously on startup.
        let mut was_active = is_active_conn(&mut conn);

        // Brief initial delay to let the VM / SPICE server settle before
        // we start watching for transitions.
        std::thread::sleep(std::time::Duration::from_secs(4));

        loop {
            std::thread::sleep(std::time::Duration::from_secs(2));
            let now_active = is_active_conn(&mut conn);

            if was_active && !now_active {
                // VM just became inactive — show powered-off page.
                unsafe {
                    g_idle_add(idle_set_powered_off, poll_viewer_addr as *mut c_void);
                }
            } else if !was_active && now_active {
                // VM just became active (snapshot revert, external power-on, …)
                // — reconnect the display.
                eprintln!("grustyvman-viewer: VM became active, reconnecting SPICE");
                if poll_viewer_addr != 0 {
                    unsafe {
                        g_idle_add(idle_set_reverting, poll_viewer_addr as *mut c_void);
                    }
                }
                if let Some(endpoint) = wait_for_spice_endpoint(&poll_vm, 20) {
                    schedule_reconnect(poll_viewer_addr, endpoint);
                } else {
                    eprintln!("grustyvman-viewer: timed out waiting for SPICE after VM activation");
                    if poll_viewer_addr != 0 {
                        unsafe {
                            g_idle_add(idle_set_powered_off, poll_viewer_addr as *mut c_void);
                        }
                    }
                }
            }

            was_active = now_active;
        }
    });

    // ── Run the GTK main loop ───────────────────────────────────────────────
    unsafe {
        gtk_main();
        // Reclaim VmControl to avoid the leak warning.
        drop(Box::from_raw(action_ctx_ptr));
    }
}
