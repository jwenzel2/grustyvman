use virt::connect::Connect;
use crate::backend::types::{VmInfo, VmState};
use crate::error::AppError;

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
