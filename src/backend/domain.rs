use virt::connect::Connect;
use virt::domain::Domain;
use crate::error::AppError;

pub(crate) fn with_domain<F, R>(uri: &str, uuid: &str, f: F) -> Result<R, AppError>
where
    F: FnOnce(&Domain) -> Result<R, AppError>,
{
    let conn = Connect::open(Some(uri))?;
    let domain = Domain::lookup_by_uuid_string(&conn, uuid)?;
    f(&domain)
}

pub fn start_vm(uri: &str, uuid: &str) -> Result<(), AppError> {
    with_domain(uri, uuid, |domain| {
        domain.create()?;
        Ok(())
    })
}

pub fn shutdown_vm(uri: &str, uuid: &str) -> Result<(), AppError> {
    with_domain(uri, uuid, |domain| {
        domain.shutdown()?;
        Ok(())
    })
}

pub fn force_stop_vm(uri: &str, uuid: &str) -> Result<(), AppError> {
    with_domain(uri, uuid, |domain| {
        domain.destroy()?;
        Ok(())
    })
}

pub fn pause_vm(uri: &str, uuid: &str) -> Result<(), AppError> {
    with_domain(uri, uuid, |domain| {
        domain.suspend()?;
        Ok(())
    })
}

pub fn resume_vm(uri: &str, uuid: &str) -> Result<(), AppError> {
    with_domain(uri, uuid, |domain| {
        domain.resume()?;
        Ok(())
    })
}

pub fn reboot_vm(uri: &str, uuid: &str) -> Result<(), AppError> {
    with_domain(uri, uuid, |domain| {
        domain.reboot(0)?;
        Ok(())
    })
}

pub fn delete_vm(uri: &str, uuid: &str) -> Result<(), AppError> {
    with_domain(uri, uuid, |domain| {
        // Try to destroy if running
        let _ = domain.destroy();
        domain.undefine()?;
        Ok(())
    })
}

pub fn get_domain_xml(uri: &str, uuid: &str) -> Result<String, AppError> {
    with_domain(uri, uuid, |domain| {
        let xml = domain.get_xml_desc(0)?;
        Ok(xml)
    })
}

pub fn get_domain_name(uri: &str, uuid: &str) -> Result<String, AppError> {
    with_domain(uri, uuid, |domain| {
        let name = domain.get_name()?;
        Ok(name)
    })
}

pub fn update_domain_xml(uri: &str, xml: &str) -> Result<(), AppError> {
    let conn = Connect::open(Some(uri))?;
    Domain::define_xml(&conn, xml)?;
    Ok(())
}

pub fn get_autostart(uri: &str, uuid: &str) -> Result<bool, AppError> {
    with_domain(uri, uuid, |domain| {
        let autostart = domain.get_autostart()?;
        Ok(autostart)
    })
}

pub fn set_autostart(uri: &str, uuid: &str, enabled: bool) -> Result<(), AppError> {
    with_domain(uri, uuid, |domain| {
        domain.set_autostart(enabled)?;
        Ok(())
    })
}

pub fn create_disk_image(path: &str, size_gib: u64) -> Result<(), AppError> {
    let output = std::process::Command::new("qemu-img")
        .args(["create", "-f", "qcow2", path, &format!("{size_gib}G")])
        .output()?;

    if !output.status.success() {
        return Err(AppError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!(
                "qemu-img failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        )));
    }
    Ok(())
}

pub fn list_networks(uri: &str) -> Result<Vec<String>, AppError> {
    let conn = Connect::open(Some(uri))?;
    let networks = conn.list_networks()?;
    Ok(networks)
}

pub fn launch_console(uri: &str, uuid: &str) -> Result<(), AppError> {
    let name = get_domain_name(uri, uuid)?;
    std::process::Command::new("virt-viewer")
        .arg("--connect")
        .arg(uri)
        .arg("--wait")
        .arg(&name)
        .spawn()?;
    Ok(())
}
