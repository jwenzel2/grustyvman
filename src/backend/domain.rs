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
    // Fetch the running domain XML (with VIR_DOMAIN_XML_SECURE=1 to get passwd)
    let xml = with_domain(uri, uuid, |domain| Ok(domain.get_xml_desc(1)?))?;
    let details = crate::backend::domain_xml::parse_domain_xml(&xml)?;

    match details.graphics {
        Some(ref g) if g.graphics_type == crate::backend::types::GraphicsType::Spice => {
            let port: i32 = match g.port {
                Some(p) if p > 0 => p,
                _ => {
                    return Err(AppError::Io(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "SPICE port not yet allocated — is the VM running?",
                    )))
                }
            };
            // Resolve the host: treat 0.0.0.0 and empty as localhost
            let host = g
                .listen_address
                .as_deref()
                .map(|a: &str| if a == "0.0.0.0" || a.is_empty() { "127.0.0.1" } else { a })
                .unwrap_or("127.0.0.1");

            let viewer = find_viewer_binary();
            eprintln!("grustyvman: launching viewer binary {}", viewer.display());
            log::debug!("Launching SPICE viewer: {}", viewer.display());
            let name = get_domain_name(uri, uuid)?;
            let mut cmd = std::process::Command::new(&viewer);
            cmd.arg("--host").arg(host)
                .arg("--port").arg(port.to_string())
                .arg("--uri").arg(uri)
                .arg("--uuid").arg(uuid)
                .arg("--title").arg(format!("{name} — SPICE Console"));
            if let Some(ref pw) = g.password {
                cmd.arg("--password").arg(pw);
            }
            cmd.spawn()?;
        }
        _ => {
            // VNC or no graphics: fall back to virt-viewer
            let name = get_domain_name(uri, uuid)?;
            std::process::Command::new("virt-viewer")
                .arg("--connect")
                .arg(uri)
                .arg("--wait")
                .arg(&name)
                .spawn()?;
        }
    }
    Ok(())
}

fn find_viewer_binary() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("GRUSTYVMAN_VIEWER") {
        let candidate = std::path::PathBuf::from(path);
        if candidate.is_file() {
            return candidate;
        }
    }

    let mut candidates: Vec<std::path::PathBuf> = Vec::new();

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(
            cwd.join("viewer")
                .join("target")
                .join("debug")
                .join("grustyvman-viewer"),
        );
        candidates.push(
            cwd.join("viewer")
                .join("target")
                .join("release")
                .join("grustyvman-viewer"),
        );
        candidates.push(cwd.join("viewer").join("grustyvman-viewer"));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if let Some(parent) = dir.parent() {
                if let Some(repo_root) = parent.parent() {
                    candidates.push(
                        repo_root
                            .join("viewer")
                            .join("target")
                            .join("debug")
                            .join("grustyvman-viewer"),
                    );
                    candidates.push(
                        repo_root
                            .join("viewer")
                            .join("target")
                            .join("release")
                            .join("grustyvman-viewer"),
                    );
                    candidates.push(repo_root.join("viewer").join("grustyvman-viewer"));
                }

                candidates.push(parent.join("debug").join("grustyvman-viewer"));
                candidates.push(parent.join("release").join("grustyvman-viewer"));
            }

            candidates.push(dir.join("grustyvman-viewer"));
        }
    }

    for candidate in candidates {
        if candidate.is_file() {
            return candidate;
        }
    }

    std::path::PathBuf::from("grustyvman-viewer")
}

/// Rename a shutoff domain by redefining it with a new name.
pub fn rename_domain(uri: &str, uuid: &str, new_name: &str) -> Result<(), AppError> {
    let xml = get_domain_xml(uri, uuid)?;
    let new_xml = crate::backend::domain_xml::rename_domain_xml(&xml, new_name)?;
    // Undefine old, define new
    with_domain(uri, uuid, |domain| {
        domain.undefine()?;
        Ok(())
    })?;
    update_domain_xml(uri, &new_xml)?;
    Ok(())
}

/// Clone a shutoff domain. If `full_clone` is true, copies disk images fully;
/// otherwise creates linked (backing-store) clones.
pub fn clone_domain(
    uri: &str,
    uuid: &str,
    new_name: &str,
    full_clone: bool,
) -> Result<(), AppError> {
    let xml = get_domain_xml(uri, uuid)?;
    let disk_paths = crate::backend::domain_xml::extract_disk_paths(&xml);

    let disk_map: Vec<(String, String)> = disk_paths
        .iter()
        .map(|path| {
            let new_path = derive_clone_path(path, new_name);
            (path.clone(), new_path)
        })
        .collect();

    for (src, dst) in &disk_map {
        let args: &[&str] = if full_clone {
            &["convert", "-f", "qcow2", "-O", "qcow2", src.as_str(), dst.as_str()]
        } else {
            &["create", "-f", "qcow2", "-F", "qcow2", "-b", src.as_str(), dst.as_str()]
        };
        let output = std::process::Command::new("qemu-img").args(args).output()?;
        if !output.status.success() {
            return Err(AppError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!(
                    "qemu-img failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            )));
        }
    }

    let new_xml = crate::backend::domain_xml::prepare_clone_xml(&xml, new_name, &disk_map)?;
    update_domain_xml(uri, &new_xml)?;
    Ok(())
}

fn derive_clone_path(original: &str, new_name: &str) -> String {
    let path = std::path::Path::new(original);
    let parent = path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string());
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    format!("{}/{}{}", parent, new_name, ext)
}
