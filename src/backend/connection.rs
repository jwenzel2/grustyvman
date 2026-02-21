use std::sync::{Mutex, OnceLock};
use virt::connect::Connect;
use crate::backend::types::{HostInfo, VmInfo, VmState};
use crate::error::AppError;

// ---------------------------------------------------------------------------
// Connection cache
//
// libvirt connections use several file descriptors each (Unix socket + internal
// pipes for the event loop). Opening a new connection on every 2-second poll
// tick exhausts the process fd limit over hours of use.
//
// Instead we cache one connection per URI. Each caller gets a cheap
// virConnectRef clone — no new socket is opened. The cache entry keeps the
// underlying connection alive between calls. On any connection error the cache
// is invalidated so the next call reconnects cleanly.
// ---------------------------------------------------------------------------

struct ConnCache {
    uri: String,
    conn: Connect,
}

static CONN_CACHE: OnceLock<Mutex<Option<ConnCache>>> = OnceLock::new();

fn conn_cache() -> &'static Mutex<Option<ConnCache>> {
    CONN_CACHE.get_or_init(|| Mutex::new(None))
}

/// Return a ref-counted clone of the cached connection for `uri`.
/// Opens a new connection (and caches it) if none exists or the existing
/// one is no longer alive.
pub fn get_conn(uri: &str) -> Result<Connect, AppError> {
    let mut guard = conn_cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    let alive = guard
        .as_ref()
        .map(|c| c.uri == uri && c.conn.is_alive().unwrap_or(false))
        .unwrap_or(false);

    if !alive {
        *guard = None;
        let conn = Connect::open(Some(uri))?;
        *guard = Some(ConnCache { uri: uri.to_string(), conn });
    }

    // Clone increments virConnectRef; the cache entry keeps the master open.
    Ok(guard.as_ref().unwrap().conn.clone())
}

/// Invalidate the cached connection (e.g. after a fatal error or explicit
/// disconnect). The next call to `get_conn` will open a fresh connection.
#[allow(dead_code)]
pub fn invalidate_conn() {
    if let Ok(mut guard) = conn_cache().lock() {
        *guard = None;
    }
}

// ---------------------------------------------------------------------------

pub fn get_host_info(uri: &str) -> Result<HostInfo, AppError> {
    let conn = get_conn(uri)?;

    let hostname = conn.get_hostname().unwrap_or_else(|_| "Unknown".to_string());
    let node_info = conn.get_node_info()?;

    let lib_version = conn.get_lib_version().unwrap_or(0);
    let lib_major = lib_version / 1_000_000;
    let lib_minor = (lib_version / 1_000) % 1_000;
    let lib_micro = lib_version % 1_000;
    let libvirt_version = format!("{lib_major}.{lib_minor}.{lib_micro}");

    let hv_version = conn.get_hyp_version().unwrap_or(0);
    let hv_major = hv_version / 1_000_000;
    let hv_minor = (hv_version / 1_000) % 1_000;
    let hv_micro = hv_version % 1_000;
    let hypervisor_version = format!("{hv_major}.{hv_minor}.{hv_micro}");

    Ok(HostInfo {
        hostname,
        uri: uri.to_string(),
        libvirt_version,
        hypervisor_version,
        cpu_model: node_info.model.clone(),
        cpu_cores: node_info.cores,
        cpu_threads: node_info.threads,
        cpu_mhz: node_info.mhz,
        cpu_sockets: node_info.sockets,
        cpu_nodes: node_info.nodes,
        memory_kib: node_info.memory,
    })
}

pub fn list_all_vms(uri: &str) -> Result<Vec<VmInfo>, AppError> {
    let conn = get_conn(uri)?;

    let flags = virt::sys::VIR_CONNECT_LIST_DOMAINS_ACTIVE
        | virt::sys::VIR_CONNECT_LIST_DOMAINS_INACTIVE;

    let domains = conn.list_all_domains(flags)?;
    let mut vms = Vec::with_capacity(domains.len());

    for domain in &domains {
        let name = domain.get_name()?;
        let uuid = domain.get_uuid_string()?;
        let info = domain.get_info()?;
        let state = VmState::from_libvirt(info.state as u32);
        let id = if info.state as u32 == 1 {
            domain.get_id()
        } else {
            None
        };

        vms.push(VmInfo {
            name,
            uuid,
            state,
            vcpus: info.nr_virt_cpu as u32,
            memory_kib: info.memory,
            id,
        });
    }

    vms.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(vms)
}
