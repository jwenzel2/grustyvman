use virt::connect::Connect;
use crate::backend::types::{HostInfo, VmInfo, VmState};
use crate::error::AppError;

pub fn get_host_info(uri: &str) -> Result<HostInfo, AppError> {
    let conn = Connect::open(Some(uri))?;

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
    let conn = Connect::open(Some(uri))?;

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
