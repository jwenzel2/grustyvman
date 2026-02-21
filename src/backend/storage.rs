use virt::storage_pool::StoragePool;
use virt::storage_vol::StorageVol;
use virt::stream::Stream;
use crate::backend::connection::get_conn;

use crate::backend::types::{PoolCreateParams, PoolInfo, PoolState, VolumeInfo, VolumeType};
use crate::error::AppError;

fn with_pool<F, R>(uri: &str, uuid: &str, f: F) -> Result<R, AppError>
where
    F: FnOnce(&StoragePool) -> Result<R, AppError>,
{
    let conn = get_conn(uri)?;
    let pool = StoragePool::lookup_by_uuid_string(&conn, uuid)?;
    f(&pool)
}

pub fn list_all_pools(uri: &str) -> Result<Vec<PoolInfo>, AppError> {
    let conn = get_conn(uri)?;
    let pools = conn.list_all_storage_pools(0)?;

    let mut result = Vec::new();
    for pool in &pools {
        let name = pool.get_name()?;
        let uuid = pool.get_uuid_string()?;
        let info = pool.get_info()?;
        let active = pool.is_active().unwrap_or(false);
        let persistent = pool.is_persistent().unwrap_or(false);
        let autostart = pool.get_autostart().unwrap_or(false);

        result.push(PoolInfo {
            name,
            uuid,
            state: PoolState::from_libvirt(info.state),
            capacity: info.capacity,
            allocation: info.allocation,
            available: info.available,
            active,
            persistent,
            autostart,
        });
    }

    result.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(result)
}

pub fn start_pool(uri: &str, uuid: &str) -> Result<(), AppError> {
    with_pool(uri, uuid, |pool| {
        pool.create(0)?;
        Ok(())
    })
}

pub fn stop_pool(uri: &str, uuid: &str) -> Result<(), AppError> {
    with_pool(uri, uuid, |pool| {
        pool.destroy()?;
        Ok(())
    })
}

pub fn delete_pool(uri: &str, uuid: &str) -> Result<(), AppError> {
    with_pool(uri, uuid, |pool| {
        let _ = pool.destroy();
        pool.undefine()?;
        Ok(())
    })
}

pub fn refresh_pool(uri: &str, uuid: &str) -> Result<(), AppError> {
    with_pool(uri, uuid, |pool| {
        pool.refresh(0)?;
        Ok(())
    })
}

pub fn set_pool_autostart(uri: &str, uuid: &str, autostart: bool) -> Result<(), AppError> {
    with_pool(uri, uuid, |pool| {
        pool.set_autostart(autostart)?;
        Ok(())
    })
}

pub fn get_pool_xml(uri: &str, uuid: &str) -> Result<String, AppError> {
    with_pool(uri, uuid, |pool| {
        let xml = pool.get_xml_desc(0)?;
        Ok(xml)
    })
}

pub fn create_pool(
    uri: &str,
    name: &str,
    pool_type: &str,
    params: &PoolCreateParams,
) -> Result<(), AppError> {
    let xml = build_pool_xml(name, pool_type, params);

    let conn = get_conn(uri)?;
    let pool = StoragePool::define_xml(&conn, &xml, 0)?;
    let _ = pool.build(0);
    pool.create(0)?;
    Ok(())
}

fn build_pool_xml(name: &str, pool_type: &str, params: &PoolCreateParams) -> String {
    let mut xml = format!("<pool type=\"{pool_type}\">\n  <name>{name}</name>\n");

    match pool_type {
        "fs" => {
            xml.push_str("  <source>\n");
            xml.push_str(&format!("    <device path=\"{}\"/>\n", params.source_device));
            if !params.source_format.is_empty() {
                xml.push_str(&format!("    <format type=\"{}\"/>\n", params.source_format));
            }
            xml.push_str("  </source>\n");
        }
        "netfs" => {
            xml.push_str("  <source>\n");
            xml.push_str(&format!("    <host name=\"{}\"/>\n", params.source_host));
            xml.push_str(&format!("    <dir path=\"{}\"/>\n", params.source_dir));
            xml.push_str(&format!("    <format type=\"{}\"/>\n", params.source_format));
            xml.push_str("  </source>\n");
        }
        "logical" => {
            xml.push_str("  <source>\n");
            xml.push_str(&format!("    <device path=\"{}\"/>\n", params.source_device));
            xml.push_str(&format!("    <name>{}</name>\n", params.source_name));
            xml.push_str("  </source>\n");
        }
        "iscsi" => {
            xml.push_str("  <source>\n");
            xml.push_str(&format!("    <host name=\"{}\"/>\n", params.source_host));
            xml.push_str(&format!("    <device path=\"{}\"/>\n", params.source_device));
            xml.push_str("  </source>\n");
        }
        "disk" => {
            xml.push_str("  <source>\n");
            xml.push_str(&format!("    <device path=\"{}\"/>\n", params.source_device));
            if !params.source_format.is_empty() {
                xml.push_str(&format!("    <format type=\"{}\"/>\n", params.source_format));
            }
            xml.push_str("  </source>\n");
        }
        _ => {} // "dir" needs no source
    }

    xml.push_str(&format!("  <target>\n    <path>{}</path>\n  </target>\n", params.target_path));
    xml.push_str("</pool>");
    xml
}

/// Returns all active pools paired with their volumes. Used by the storage volume picker in
/// the VM config dialog so images can be chosen via libvirt without requiring direct filesystem
/// access (the directory may be owned by root/qemu).
pub fn list_all_pool_volumes(uri: &str) -> Result<Vec<(String, Vec<VolumeInfo>)>, AppError> {
    let conn = get_conn(uri)?;
    let pools = conn.list_all_storage_pools(0)?;

    let mut result = Vec::new();
    for pool in &pools {
        if !pool.is_active().unwrap_or(false) {
            continue;
        }
        let name = pool.get_name().unwrap_or_default();
        let vol_names = pool.list_volumes().unwrap_or_default();
        let mut volumes = Vec::new();
        for vol_name in &vol_names {
            if let Ok(vol) = StorageVol::lookup_by_name(pool, vol_name) {
                let path = vol.get_path().unwrap_or_default();
                let info = vol.get_info().unwrap_or(virt::storage_vol::StorageVolInfo {
                    kind: 0,
                    capacity: 0,
                    allocation: 0,
                });
                volumes.push(VolumeInfo {
                    name: vol_name.clone(),
                    path,
                    kind: VolumeType::from_libvirt(info.kind),
                    capacity: info.capacity,
                    allocation: info.allocation,
                });
            }
        }
        volumes.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        result.push((name, volumes));
    }
    result.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    Ok(result)
}

pub fn list_pool_volumes(uri: &str, pool_uuid: &str) -> Result<Vec<VolumeInfo>, AppError> {
    with_pool(uri, pool_uuid, |pool| {
        let vol_names = pool.list_volumes().unwrap_or_default();
        let mut volumes = Vec::new();

        for vol_name in &vol_names {
            if let Ok(vol) = StorageVol::lookup_by_name(pool, vol_name) {
                let path = vol.get_path().unwrap_or_default();
                let info = vol.get_info().unwrap_or(virt::storage_vol::StorageVolInfo {
                    kind: 0,
                    capacity: 0,
                    allocation: 0,
                });

                volumes.push(VolumeInfo {
                    name: vol_name.clone(),
                    path,
                    kind: VolumeType::from_libvirt(info.kind),
                    capacity: info.capacity,
                    allocation: info.allocation,
                });
            }
        }

        volumes.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(volumes)
    })
}

pub fn create_volume(
    uri: &str,
    pool_uuid: &str,
    name: &str,
    capacity_bytes: u64,
    format: &str,
) -> Result<(), AppError> {
    with_pool(uri, pool_uuid, |pool| {
        let xml = format!(
            r#"<volume>
  <name>{name}</name>
  <capacity unit="bytes">{capacity_bytes}</capacity>
  <target>
    <format type="{format}"/>
  </target>
</volume>"#
        );

        StorageVol::create_xml(pool, &xml, 0)?;
        Ok(())
    })
}

pub fn delete_volume(uri: &str, pool_uuid: &str, vol_name: &str) -> Result<(), AppError> {
    with_pool(uri, pool_uuid, |pool| {
        let vol = StorageVol::lookup_by_name(pool, vol_name)?;
        vol.delete(0)?;
        Ok(())
    })
}

/// Create a disk volume for a new VM in the default storage pool (falling back
/// to the first active pool). Returns the absolute path to the new volume.
/// Uses libvirt so the daemon handles permissions — no direct filesystem access needed.
pub fn create_vm_disk(
    uri: &str,
    name: &str,
    capacity_gib: u64,
    format: &str,
    extension: &str,
) -> Result<String, AppError> {
    let conn = get_conn(uri)?;
    let pools = conn.list_all_storage_pools(0)?;

    // Prefer "default"; fall back to the first active pool.
    let pool = pools
        .iter()
        .find(|p| {
            p.is_active().unwrap_or(false)
                && p.get_name().map(|n| n == "default").unwrap_or(false)
        })
        .or_else(|| pools.iter().find(|p| p.is_active().unwrap_or(false)))
        .ok_or_else(|| AppError::Libvirt("No active storage pool found".to_string()))?;

    let vol_name = format!("{name}.{extension}");
    let capacity_bytes = capacity_gib * 1024 * 1024 * 1024;
    let xml = format!(
        r#"<volume>
  <name>{vol_name}</name>
  <capacity unit="bytes">{capacity_bytes}</capacity>
  <target>
    <format type="{format}"/>
  </target>
</volume>"#
    );

    let vol = StorageVol::create_xml(pool, &xml, 0)?;
    let path = vol.get_path()?;
    Ok(path)
}

/// Delete a storage volume by its absolute path. Ignores errors from volumes
/// that are not tracked by any pool (e.g. manually placed files).
pub fn delete_volume_by_path(uri: &str, path: &str) -> Result<(), AppError> {
    let conn = get_conn(uri)?;
    let vol = StorageVol::lookup_by_path(&conn, path)?;
    vol.delete(0)?;
    Ok(())
}

/// Upload a local file into a storage pool volume via the libvirt stream API.
/// The daemon handles file creation and permissions — no direct filesystem
/// access required, so this works even when the pool directory is root-owned.
pub fn upload_volume(
    uri: &str,
    pool_uuid: &str,
    src_path: &str,
    vol_name: &str,
) -> Result<(), AppError> {
    use std::io::Read;

    let file_size = std::fs::metadata(src_path)
        .map_err(|e| AppError::Io(e))?
        .len();

    let format = if src_path.ends_with(".qcow2") { "qcow2" } else { "raw" };

    let conn = get_conn(uri)?;
    let pool = StoragePool::lookup_by_uuid_string(&conn, pool_uuid)?;

    let xml = format!(
        r#"<volume>
  <name>{vol_name}</name>
  <capacity unit="bytes">{file_size}</capacity>
  <target>
    <format type="{format}"/>
  </target>
</volume>"#
    );
    let vol = StorageVol::create_xml(&pool, &xml, 0)?;

    let stream = Stream::new(&conn, 0).map_err(|e| AppError::Libvirt(e.to_string()))?;
    vol.upload(&stream, 0, file_size, 0)?;

    let send_result: Result<(), AppError> = (|| {
        let mut file = std::fs::File::open(src_path).map_err(|e| AppError::Io(e))?;
        let mut buf = vec![0u8; 256 * 1024];
        loop {
            let n = file.read(&mut buf).map_err(|e| AppError::Io(e))?;
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
            stream.finish().map_err(|e| AppError::Libvirt(e.to_string()))?;
            Ok(())
        }
        Err(e) => {
            let _ = stream.abort();
            let _ = vol.delete(0);
            Err(e)
        }
    }
}

pub fn extract_pool_type_and_path(xml: &str) -> (String, String) {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut pool_type = String::new();
    let mut path = String::new();
    let mut in_target = false;
    let mut in_path = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "pool" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"type" {
                                pool_type =
                                    String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    }
                    "target" => in_target = true,
                    "path" if in_target => in_path = true,
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_path {
                    path = e.unescape().unwrap_or_default().to_string();
                    in_path = false;
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "target" {
                    in_target = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    (pool_type, path)
}
