use std::path::{Path, PathBuf};
use virt::domain::Domain;
use virt::domain_snapshot::DomainSnapshot;
use virt::storage_pool::StoragePool;
use virt::storage_vol::StorageVol;
use virt::stream::Stream;

use crate::backend::connection::get_conn;
use crate::backend::domain::{get_domain_xml, with_domain};
use crate::backend::domain_xml::extract_disk_paths;
use crate::error::AppError;

/// Backup a shut-off VM's XML, disk images, and snapshot metadata into a `.tar.gz` archive.
/// Disk images are exported via the libvirt stream API so no direct filesystem access is needed.
pub fn backup_vm(uri: &str, uuid: &str, dest_path: &str) -> Result<(), AppError> {
    // Ensure VM is shut off
    let state = with_domain(uri, uuid, |domain| {
        let info = domain.get_info()?;
        Ok(info.state)
    })?;
    if state != virt::sys::VIR_DOMAIN_SHUTOFF {
        return Err(AppError::Libvirt(
            "VM must be shut off before backup".to_string(),
        ));
    }

    let xml = get_domain_xml(uri, uuid)?;
    let disk_paths = extract_disk_paths(&xml);

    // Collect snapshot XMLs
    let snapshot_xmls: Vec<(String, String)> = with_domain(uri, uuid, |domain| {
        let snaps = domain.list_all_snapshots(0)?;
        let mut result = Vec::new();
        for snap in &snaps {
            let name = snap.get_name()?;
            let snap_xml = snap.get_xml_desc(0)?;
            result.push((name, snap_xml));
        }
        Ok(result)
    })?;

    // Get domain name for manifest
    let vm_name = crate::backend::domain::get_domain_name(uri, uuid)?;

    // Create temp directory
    let tmp_dir = tempdir()?;
    let base = tmp_dir.join("vm-backup");
    std::fs::create_dir_all(base.join("disks"))?;
    std::fs::create_dir_all(base.join("snapshots"))?;

    // Write domain XML
    std::fs::write(base.join("domain.xml"), &xml)?;

    // Write snapshot XMLs
    let snap_names: Vec<String> = snapshot_xmls
        .iter()
        .map(|(name, _)| name.clone())
        .collect();
    for (name, snap_xml) in &snapshot_xmls {
        let safe_name = sanitize_filename(name);
        std::fs::write(
            base.join("snapshots").join(format!("{safe_name}.xml")),
            snap_xml,
        )?;
    }

    // Download disk images via libvirt stream API
    let conn = get_conn(uri)?;
    let disk_filenames: Vec<String> = disk_paths
        .iter()
        .map(|p| {
            Path::new(p)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        })
        .collect();
    for path in &disk_paths {
        let fname = Path::new(path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let dest = base.join("disks").join(&fname);
        download_volume(&conn, path, &dest)?;
    }

    // Write manifest.json (hand-written to avoid serde dependency)
    let disks_json: Vec<String> = disk_filenames
        .iter()
        .map(|d| format!("\"{}\"", escape_json(d)))
        .collect();
    let snaps_json: Vec<String> = snap_names
        .iter()
        .map(|s| format!("\"{}\"", escape_json(s)))
        .collect();
    let manifest = format!(
        "{{\n  \"version\": 1,\n  \"name\": \"{}\",\n  \"date\": \"{}\",\n  \"disks\": [{}],\n  \"snapshots\": [{}]\n}}\n",
        escape_json(&vm_name),
        chrono_now(),
        disks_json.join(", "),
        snaps_json.join(", "),
    );
    std::fs::write(base.join("manifest.json"), &manifest)?;

    // Create tar.gz
    let output = std::process::Command::new("tar")
        .args([
            "czf",
            dest_path,
            "-C",
            &tmp_dir.to_string_lossy(),
            "vm-backup",
        ])
        .output()?;
    if !output.status.success() {
        return Err(AppError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("tar failed: {}", String::from_utf8_lossy(&output.stderr)),
        )));
    }

    // Clean up
    let _ = std::fs::remove_dir_all(&tmp_dir);

    Ok(())
}

/// Restore a VM from a `.tar.gz` backup archive. Returns the new VM name.
/// Disk images are uploaded via the libvirt stream API so no direct filesystem access is needed.
pub fn restore_vm(uri: &str, archive_path: &str) -> Result<String, AppError> {
    let tmp_dir = tempdir()?;

    // Extract archive
    let output = std::process::Command::new("tar")
        .args(["xzf", archive_path, "-C", &tmp_dir.to_string_lossy()])
        .output()?;
    if !output.status.success() {
        return Err(AppError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!(
                "tar extract failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        )));
    }

    let base = tmp_dir.join("vm-backup");
    if !base.exists() {
        return Err(AppError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Invalid backup archive: missing vm-backup directory",
        )));
    }

    // Read domain XML
    let xml = std::fs::read_to_string(base.join("domain.xml"))?;
    let disk_paths = extract_disk_paths(&xml);

    // Read manifest to get name
    let manifest_str = std::fs::read_to_string(base.join("manifest.json"))?;
    let vm_name = parse_manifest_name(&manifest_str);

    // Find target storage pool
    let conn = get_conn(uri)?;
    let pool = find_active_pool(&conn)?;

    // Upload disk images via libvirt stream API and build disk path map
    let mut disk_map: Vec<(String, String)> = Vec::new();
    for old_path in &disk_paths {
        let fname = Path::new(old_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let src = base.join("disks").join(&fname);

        if src.exists() {
            let new_path = upload_volume_to_pool(&conn, &pool, &src, &fname)?;
            disk_map.push((old_path.clone(), new_path));
        }
    }

    // Determine final VM name (avoid conflicts)
    let final_name = unique_vm_name(&conn, &vm_name)?;

    // Update XML with new disk paths and name
    let new_xml =
        crate::backend::domain_xml::prepare_clone_xml(&xml, &final_name, &disk_map)?;

    // Define the domain
    let domain = Domain::define_xml(&conn, &new_xml)?;

    // Restore snapshot metadata
    let snap_dir = base.join("snapshots");
    if snap_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&snap_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("xml") {
                    if let Ok(snap_xml) = std::fs::read_to_string(&path) {
                        let updated_snap_xml =
                            replace_disk_paths_in_xml(&snap_xml, &disk_map);
                        let flags = virt::sys::VIR_DOMAIN_SNAPSHOT_CREATE_REDEFINE;
                        let _ = DomainSnapshot::create_xml(
                            &domain,
                            &updated_snap_xml,
                            flags,
                        );
                    }
                }
            }
        }
    }

    // Clean up
    let _ = std::fs::remove_dir_all(&tmp_dir);

    // Refresh storage pools so libvirt sees the new volumes
    refresh_pools(&conn);

    Ok(final_name)
}

/// Download a storage volume by path to a local file via the libvirt stream API.
fn download_volume(
    conn: &virt::connect::Connect,
    vol_path: &str,
    dest: &Path,
) -> Result<(), AppError> {
    use std::io::Write;

    let vol = StorageVol::lookup_by_path(conn, vol_path)?;
    let info = vol.get_info()?;
    let capacity = info.capacity;

    let stream =
        Stream::new(conn, 0).map_err(|e| AppError::Libvirt(e.to_string()))?;
    vol.download(&stream, 0, capacity, 0)?;

    let recv_result: Result<(), AppError> = (|| {
        let mut file = std::fs::File::create(dest)?;
        let mut buf = vec![0u8; 256 * 1024];
        loop {
            let n = stream
                .recv(&mut buf)
                .map_err(|e| AppError::Libvirt(e.to_string()))?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])?;
        }
        file.flush()?;
        Ok(())
    })();

    match recv_result {
        Ok(()) => {
            stream
                .finish()
                .map_err(|e| AppError::Libvirt(e.to_string()))?;
            Ok(())
        }
        Err(e) => {
            let _ = stream.abort();
            Err(e)
        }
    }
}

/// Upload a local file into a storage pool as a new volume via the libvirt stream API.
/// Returns the volume's path within the pool.
fn upload_volume_to_pool(
    conn: &virt::connect::Connect,
    pool: &StoragePool,
    src: &Path,
    vol_name: &str,
) -> Result<String, AppError> {
    use std::io::Read;

    let file_size = std::fs::metadata(src)?.len();
    let format = if vol_name.ends_with(".qcow2") {
        "qcow2"
    } else {
        "raw"
    };

    let xml = format!(
        r#"<volume>
  <name>{vol_name}</name>
  <capacity unit="bytes">{file_size}</capacity>
  <target>
    <format type="{format}"/>
  </target>
</volume>"#
    );
    let vol = StorageVol::create_xml(pool, &xml, 0)?;

    let stream =
        Stream::new(conn, 0).map_err(|e| AppError::Libvirt(e.to_string()))?;
    vol.upload(&stream, 0, file_size, 0)?;

    let send_result: Result<(), AppError> = (|| {
        let mut file = std::fs::File::open(src)?;
        let mut buf = vec![0u8; 256 * 1024];
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            let mut sent = 0;
            while sent < n {
                let s = stream
                    .send(&buf[sent..n])
                    .map_err(|e| AppError::Libvirt(e.to_string()))?;
                sent += s;
            }
        }
        Ok(())
    })();

    match send_result {
        Ok(()) => {
            stream
                .finish()
                .map_err(|e| AppError::Libvirt(e.to_string()))?;
            let path = vol.get_path()?;
            Ok(path)
        }
        Err(e) => {
            let _ = stream.abort();
            let _ = vol.delete(0);
            Err(e)
        }
    }
}

/// Find the "default" active storage pool, or fall back to the first active pool.
fn find_active_pool(
    conn: &virt::connect::Connect,
) -> Result<StoragePool, AppError> {
    let pools = conn.list_all_storage_pools(0)?;

    let pool = pools
        .into_iter()
        .find(|p| {
            p.is_active().unwrap_or(false)
                && p.get_name().map(|n| n == "default").unwrap_or(false)
        });

    if let Some(p) = pool {
        return Ok(p);
    }

    // Re-fetch since into_iter consumed them
    let pools = conn.list_all_storage_pools(0)?;
    pools
        .into_iter()
        .find(|p| p.is_active().unwrap_or(false))
        .ok_or_else(|| AppError::Libvirt("No active storage pool found".to_string()))
}

/// Create a temporary directory under /tmp.
fn tempdir() -> Result<PathBuf, std::io::Error> {
    let name = format!("grustyvman-backup-{}", std::process::id());
    let path = std::env::temp_dir().join(name);
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

/// Get current date as ISO string without external crate.
fn chrono_now() -> String {
    let output = std::process::Command::new("date")
        .args(["+%Y-%m-%dT%H:%M:%S"])
        .output();
    match output {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        }
        _ => "unknown".to_string(),
    }
}

/// Escape a string for JSON embedding.
fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Make a filename safe by replacing path separators.
fn sanitize_filename(name: &str) -> String {
    name.replace('/', "_").replace('\\', "_")
}

/// Parse the VM name from a simple manifest.json.
fn parse_manifest_name(json: &str) -> String {
    if let Some(idx) = json.find("\"name\"") {
        let rest = &json[idx + 6..];
        if let Some(colon) = rest.find(':') {
            let after_colon = &rest[colon + 1..];
            if let Some(q1) = after_colon.find('"') {
                let after_q1 = &after_colon[q1 + 1..];
                if let Some(q2) = after_q1.find('"') {
                    return after_q1[..q2].to_string();
                }
            }
        }
    }
    "restored-vm".to_string()
}

/// If a VM with the given name already exists, append "-restored" (or "-restored-N").
fn unique_vm_name(
    conn: &virt::connect::Connect,
    base_name: &str,
) -> Result<String, AppError> {
    let existing: Vec<String> = conn
        .list_all_domains(0)?
        .iter()
        .filter_map(|d| d.get_name().ok())
        .collect();

    if !existing.contains(&base_name.to_string()) {
        return Ok(base_name.to_string());
    }

    let candidate = format!("{base_name}-restored");
    if !existing.contains(&candidate) {
        return Ok(candidate);
    }

    for i in 2..100 {
        let candidate = format!("{base_name}-restored-{i}");
        if !existing.contains(&candidate) {
            return Ok(candidate);
        }
    }

    Ok(format!("{base_name}-restored-{}", std::process::id()))
}

/// Replace disk source file paths within any XML string (used for snapshot XML).
fn replace_disk_paths_in_xml(xml: &str, disk_map: &[(String, String)]) -> String {
    let mut result = xml.to_string();
    for (old, new) in disk_map {
        result = result.replace(old.as_str(), new.as_str());
    }
    result
}

/// Refresh all active storage pools so libvirt picks up newly created volumes.
fn refresh_pools(conn: &virt::connect::Connect) {
    if let Ok(pools) = conn.list_all_storage_pools(0) {
        for pool in &pools {
            if pool.is_active().unwrap_or(false) {
                let _ = pool.refresh(0);
            }
        }
    }
}
