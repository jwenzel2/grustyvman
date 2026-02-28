use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use quick_xml::events::Event;
use quick_xml::Reader;
use virt::domain::Domain;
use virt::storage_pool::StoragePool;

use crate::backend::backup::{upload_volume_to_pool, unique_vm_name, ProgressFn};
use crate::backend::connection::get_conn;
use crate::backend::types::{ConvertNicConfig, DiskFormat, FirmwareType, NetworkSourceType};
use crate::backend::vmware::{tempdir, local_name, attr_val};
use crate::error::AppError;

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct VBoxDisk {
    pub source_path: String,
    pub format: String,   // "vdi", "vmdk", "vhd"
    pub bus_type: String,  // "sata", "ide", "scsi"
}

#[derive(Debug, Clone)]
pub struct VBoxNic {
    pub mac_address: Option<String>,
    pub attachment_type: String, // "nat", "bridged", "internal", "hostonly", "natnetwork"
    pub adapter_type: String,    // "Am79C973", "82540EM", "virtio", etc.
}

#[derive(Debug, Clone)]
pub struct VBoxConfig {
    pub name: String,
    pub vcpus: u32,
    pub memory_mib: u64,
    pub firmware: FirmwareType,
    pub disks: Vec<VBoxDisk>,
    pub nics: Vec<VBoxNic>,
    pub guest_os: Option<String>,
    pub source_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ConvertVBoxParams {
    pub vbox_config: VBoxConfig,
    pub target_name: String,
    pub disk_format: DiskFormat,
    pub pool_name: String,
    pub nic_configs: Vec<ConvertNicConfig>,
}

// ---------------------------------------------------------------------------
// .vbox XML parser
// ---------------------------------------------------------------------------

pub fn parse_vbox(vbox_path: &str) -> Result<VBoxConfig, AppError> {
    let content = std::fs::read_to_string(vbox_path)?;
    let source_dir = Path::new(vbox_path)
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();

    let mut reader = Reader::from_str(&content);
    reader.config_mut().trim_text(true);

    let mut name = String::new();
    let mut vcpus: u32 = 1;
    let mut memory_mib: u64 = 1024;
    let mut firmware = FirmwareType::Bios;
    let mut guest_os: Option<String> = None;
    let mut disks: Vec<VBoxDisk> = Vec::new();
    let mut nics: Vec<VBoxNic> = Vec::new();

    // Media registry: uuid -> (location, format)
    let mut media_registry: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new();

    // Storage controller: name -> bus_type
    let mut controller_types: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    // Attached disks: (controller_name, image_uuid)
    let mut attached_disks: Vec<(String, String)> = Vec::new();

    let mut current_controller = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local = local_name(e.name().as_ref());
                match local.as_str() {
                    "Machine" => {
                        for attr in e.attributes().flatten() {
                            let aname = local_name(attr.key.as_ref());
                            let aval = attr_val(&attr);
                            match aname.as_str() {
                                "name" => name = aval,
                                "OSType" => guest_os = Some(aval),
                                _ => {}
                            }
                        }
                    }
                    "CPU" => {
                        for attr in e.attributes().flatten() {
                            let aname = local_name(attr.key.as_ref());
                            if aname == "count" {
                                if let Ok(v) = attr_val(&attr).parse() {
                                    vcpus = v;
                                }
                            }
                        }
                    }
                    "Memory" => {
                        for attr in e.attributes().flatten() {
                            let aname = local_name(attr.key.as_ref());
                            if aname == "RAMSize" {
                                if let Ok(v) = attr_val(&attr).parse() {
                                    memory_mib = v;
                                }
                            }
                        }
                    }
                    "Firmware" => {
                        for attr in e.attributes().flatten() {
                            let aname = local_name(attr.key.as_ref());
                            if aname == "type" {
                                let aval = attr_val(&attr);
                                if aval.to_lowercase() == "efi" || aval.to_lowercase() == "efi64" {
                                    firmware = FirmwareType::Efi;
                                }
                            }
                        }
                    }
                    "HardDisk" => {
                        let mut uuid = String::new();
                        let mut location = String::new();
                        let mut format = String::new();
                        for attr in e.attributes().flatten() {
                            let aname = local_name(attr.key.as_ref());
                            let aval = attr_val(&attr);
                            match aname.as_str() {
                                "uuid" => uuid = normalize_uuid(&aval),
                                "location" => location = aval,
                                "format" => format = aval.to_lowercase(),
                                _ => {}
                            }
                        }
                        if !uuid.is_empty() && !location.is_empty() {
                            if format.is_empty() {
                                format = detect_vbox_disk_format(&location).to_string();
                            }
                            media_registry.insert(uuid, (location, format));
                        }
                    }
                    "StorageController" => {
                        let mut ctrl_name = String::new();
                        let mut ctrl_type = String::new();
                        for attr in e.attributes().flatten() {
                            let aname = local_name(attr.key.as_ref());
                            let aval = attr_val(&attr);
                            match aname.as_str() {
                                "name" => ctrl_name = aval,
                                "type" => ctrl_type = aval,
                                _ => {}
                            }
                        }
                        let bus = match ctrl_type.as_str() {
                            "AHCI" | "IntelAhci" => "sata",
                            "PIIX4" | "PIIX3" | "ICH6" => "ide",
                            "LsiLogic" | "BusLogic" | "LsiLogicSas" => "scsi",
                            "NVMe" => "nvme",
                            "VirtioSCSI" => "scsi",
                            _ => "sata",
                        };
                        if !ctrl_name.is_empty() {
                            current_controller = ctrl_name.clone();
                            controller_types.insert(ctrl_name, bus.to_string());
                        }
                    }
                    "AttachedDevice" => {
                        let mut is_hard_disk = false;
                        for attr in e.attributes().flatten() {
                            let aname = local_name(attr.key.as_ref());
                            if aname == "type" && attr_val(&attr) == "HardDisk" {
                                is_hard_disk = true;
                            }
                        }
                        if !is_hard_disk {
                            // Skip non-disk devices (DVD, etc.)
                            current_controller = String::new();
                        }
                    }
                    "Image" => {
                        if !current_controller.is_empty() {
                            for attr in e.attributes().flatten() {
                                let aname = local_name(attr.key.as_ref());
                                if aname == "uuid" {
                                    let uuid = normalize_uuid(&attr_val(&attr));
                                    attached_disks
                                        .push((current_controller.clone(), uuid));
                                }
                            }
                        }
                    }
                    "Adapter" => {
                        let mut enabled = false;
                        let mut mac = None;
                        let mut adapter_type = String::from("82540EM");
                        for attr in e.attributes().flatten() {
                            let aname = local_name(attr.key.as_ref());
                            let aval = attr_val(&attr);
                            match aname.as_str() {
                                "enabled" => enabled = aval == "true",
                                "MACAddress" => {
                                    if !aval.is_empty() {
                                        mac = Some(format_mac(&aval));
                                    }
                                }
                                "type" => adapter_type = aval,
                                _ => {}
                            }
                        }
                        if enabled {
                            // We'll fill attachment_type when we see the child element.
                            // Push a placeholder and update it.
                            nics.push(VBoxNic {
                                mac_address: mac,
                                attachment_type: "nat".to_string(),
                                adapter_type,
                            });
                        }
                    }
                    "NAT" => {
                        if let Some(nic) = nics.last_mut() {
                            nic.attachment_type = "nat".to_string();
                        }
                    }
                    "BridgedInterface" => {
                        if let Some(nic) = nics.last_mut() {
                            nic.attachment_type = "bridged".to_string();
                        }
                    }
                    "InternalNetwork" => {
                        if let Some(nic) = nics.last_mut() {
                            nic.attachment_type = "internal".to_string();
                        }
                    }
                    "HostOnlyInterface" => {
                        if let Some(nic) = nics.last_mut() {
                            nic.attachment_type = "hostonly".to_string();
                        }
                    }
                    "NATNetwork" => {
                        if let Some(nic) = nics.last_mut() {
                            nic.attachment_type = "natnetwork".to_string();
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let local = local_name(e.name().as_ref());
                if local == "AttachedDevice" {
                    // Reset controller tracking for non-disk case
                }
                if local == "StorageController" {
                    current_controller = String::new();
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    // Resolve attached disks to full paths
    for (ctrl_name, uuid) in &attached_disks {
        if let Some((location, format)) = media_registry.get(uuid) {
            let bus_type = controller_types
                .get(ctrl_name)
                .cloned()
                .unwrap_or_else(|| "sata".to_string());

            let full_path = if Path::new(location).is_absolute() {
                location.clone()
            } else {
                source_dir.join(location).to_string_lossy().to_string()
            };

            disks.push(VBoxDisk {
                source_path: full_path,
                format: format.clone(),
                bus_type,
            });
        }
    }

    if name.is_empty() {
        name = Path::new(vbox_path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
    }

    Ok(VBoxConfig {
        name,
        vcpus,
        memory_mib,
        firmware,
        disks,
        nics,
        guest_os,
        source_dir,
    })
}

// ---------------------------------------------------------------------------
// Conversion engine
// ---------------------------------------------------------------------------

pub fn convert_vbox_vm(
    uri: &str,
    params: &ConvertVBoxParams,
    progress: &ProgressFn,
) -> Result<String, AppError> {
    let num_disks = params.vbox_config.disks.len();
    if num_disks == 0 {
        return Err(AppError::Libvirt(
            "No disks found in VirtualBox configuration".to_string(),
        ));
    }

    progress(0.0, "Preparing conversion...");

    let conn = get_conn(uri)?;
    let final_name = unique_vm_name(&conn, &params.target_name)?;

    let tmp_dir = tempdir("grustyvman-vbox-convert")?;

    // Step 1: Convert disk images in parallel via qemu-img
    progress(0.02, "Converting disk images...");
    let format_str = params.disk_format.as_str();
    let ext = params.disk_format.extension();
    let completed = AtomicUsize::new(0);

    let converted_paths: Vec<PathBuf> = std::thread::scope(|s| {
        let handles: Vec<_> = params
            .vbox_config
            .disks
            .iter()
            .enumerate()
            .map(|(i, disk)| {
                let tmp = &tmp_dir;
                let completed = &completed;
                let src_format = &disk.format;
                s.spawn(move || {
                    let out_name = format!("disk-{i}.{ext}");
                    let out_path = tmp.join(&out_name);
                    progress(
                        0.02,
                        &format!("Converting disk {}/{}...", i + 1, num_disks),
                    );

                    let output = std::process::Command::new("qemu-img")
                        .args([
                            "convert",
                            "-f",
                            src_format,
                            "-O",
                            format_str,
                            &disk.source_path,
                            &out_path.to_string_lossy(),
                        ])
                        .output()?;
                    if !output.status.success() {
                        return Err(AppError::Io(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!(
                                "qemu-img convert failed: {}",
                                String::from_utf8_lossy(&output.stderr)
                            ),
                        )));
                    }

                    let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
                    let frac = 0.02 + 0.40 * (done as f64 / num_disks as f64);
                    progress(frac, &format!("Converted disk {done}/{num_disks}"));
                    Ok(out_path)
                })
            })
            .collect();

        let mut paths = Vec::new();
        for handle in handles {
            let path = handle
                .join()
                .map_err(|_| AppError::Libvirt("Disk conversion thread panicked".to_string()))??;
            paths.push(path);
        }
        Ok::<Vec<PathBuf>, AppError>(paths)
    })?;

    // Step 2: Upload converted images to pool in parallel
    progress(0.45, "Uploading disk images to storage pool...");
    let _pool = StoragePool::lookup_by_name(&conn, &params.pool_name)?;
    let uploaded_completed = AtomicUsize::new(0);

    let vol_names: Vec<String> = (0..num_disks)
        .map(|i| {
            if num_disks == 1 {
                format!("{}.{ext}", final_name)
            } else {
                format!("{}-disk{i}.{ext}", final_name)
            }
        })
        .collect();

    let disk_paths: Vec<String> = std::thread::scope(|s| {
        let handles: Vec<_> = converted_paths
            .iter()
            .enumerate()
            .map(|(i, src)| {
                let pool_name = &params.pool_name;
                let vol_name = &vol_names[i];
                let uploaded_completed = &uploaded_completed;
                s.spawn(move || {
                    let conn = get_conn(uri)?;
                    let pool = StoragePool::lookup_by_name(&conn, pool_name)?;
                    progress(
                        0.45,
                        &format!("Uploading disk {}/{}...", i + 1, num_disks),
                    );
                    let path = upload_volume_to_pool(&conn, &pool, src, vol_name, |_| {})?;
                    let done = uploaded_completed.fetch_add(1, Ordering::Relaxed) + 1;
                    let frac = 0.45 + 0.40 * (done as f64 / num_disks as f64);
                    progress(frac, &format!("Uploaded disk {done}/{num_disks}"));
                    Ok::<String, AppError>(path)
                })
            })
            .collect();

        let mut paths = Vec::new();
        for handle in handles {
            let path = handle
                .join()
                .map_err(|_| AppError::Libvirt("Disk upload thread panicked".to_string()))??;
            paths.push(path);
        }
        Ok::<Vec<String>, AppError>(paths)
    })?;

    // Step 3: Generate domain XML and define
    progress(0.88, "Defining VM...");
    let xml = generate_vbox_domain_xml(&final_name, params, &disk_paths);
    let _domain = Domain::define_xml(&conn, &xml)?;

    // Step 4: Refresh pools and clean up
    progress(0.95, "Cleaning up...");
    if let Ok(pools) = conn.list_all_storage_pools(0) {
        for p in &pools {
            if p.is_active().unwrap_or(false) {
                let _ = p.refresh(0);
            }
        }
    }
    let _ = std::fs::remove_dir_all(&tmp_dir);

    progress(1.0, "Conversion complete");
    Ok(final_name)
}

// ---------------------------------------------------------------------------
// Domain XML generation
// ---------------------------------------------------------------------------

fn generate_vbox_domain_xml(
    name: &str,
    params: &ConvertVBoxParams,
    disk_paths: &[String],
) -> String {
    let memory_kib = params.vbox_config.memory_mib * 1024;
    let vcpus = params.vbox_config.vcpus;

    let os_tag = match params.vbox_config.firmware {
        FirmwareType::Efi => r#"<os firmware="efi">"#,
        FirmwareType::Bios => "<os>",
    };

    let smm_feature = match params.vbox_config.firmware {
        FirmwareType::Efi => "\n    <smm state=\"on\"/>",
        FirmwareType::Bios => "",
    };

    let disk_format = params.disk_format.as_str();
    let disks_xml: String = disk_paths
        .iter()
        .enumerate()
        .map(|(i, path)| {
            let dev = format!("vd{}", (b'a' + i as u8) as char);
            format!(
                "    <disk type=\"file\" device=\"disk\">\n      <driver name=\"qemu\" type=\"{disk_format}\"/>\n      <source file=\"{path}\"/>\n      <target dev=\"{dev}\" bus=\"virtio\"/>\n    </disk>"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let nics_xml: String = params
        .nic_configs
        .iter()
        .map(|nic| {
            let (iface_type, source_elem) = match nic.source_type {
                NetworkSourceType::VirtualNetwork => (
                    "network",
                    format!(r#"<source network="{}"/>"#, nic.source_value),
                ),
                NetworkSourceType::Bridge => (
                    "bridge",
                    format!(r#"<source bridge="{}"/>"#, nic.source_value),
                ),
                NetworkSourceType::Macvtap => (
                    "direct",
                    format!(r#"<source dev="{}" mode="vepa"/>"#, nic.source_value),
                ),
                NetworkSourceType::Vdpa => (
                    "vdpa",
                    format!(r#"<source dev="{}"/>"#, nic.source_value),
                ),
            };
            let mac_elem = match &nic.mac_address {
                Some(mac) => format!("\n      <mac address=\"{mac}\"/>"),
                None => String::new(),
            };
            format!(
                "    <interface type=\"{iface_type}\">{mac_elem}\n      {source_elem}\n      <model type=\"virtio\"/>\n    </interface>"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"<domain type="kvm">
  <name>{name}</name>
  <memory unit="KiB">{memory_kib}</memory>
  <vcpu placement="static">{vcpus}</vcpu>
  {os_tag}
    <type arch="x86_64" machine="q35">hvm</type>
    <boot dev="hd"/>
  </os>
  <features>
    <acpi/>
    <apic/>{smm_feature}
  </features>
  <cpu mode="host-passthrough"/>
  <devices>
    <emulator>/usr/bin/qemu-system-x86_64</emulator>
{disks_xml}
{nics_xml}
    <sound model="ac97"/>
    <graphics type="spice" autoport="yes"/>
    <video>
      <model type="virtio"/>
    </video>
    <channel type="unix">
      <target type="virtio" name="org.qemu.guest_agent.0"/>
    </channel>
    <channel type="spicevmc">
      <target type="virtio" name="com.redhat.spice.0"/>
    </channel>
    <input type="tablet" bus="usb"/>
    <console type="pty"/>
  </devices>
</domain>"#,
    )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn detect_vbox_disk_format(path: &str) -> &str {
    match Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .as_deref()
    {
        Some("vdi") => "vdi",
        Some("vmdk") => "vmdk",
        Some("vhd") | Some("vhdx") => "vpc",
        _ => "raw",
    }
}

/// Strip curly braces from VirtualBox UUIDs: {uuid} -> uuid
fn normalize_uuid(s: &str) -> String {
    s.trim_start_matches('{')
        .trim_end_matches('}')
        .to_lowercase()
}

/// Format a bare MAC address (e.g. "080027A1B2C3") into colon-separated form
fn format_mac(raw: &str) -> String {
    if raw.contains(':') || raw.contains('-') {
        return raw.to_string();
    }
    let bytes: Vec<&str> = (0..raw.len())
        .step_by(2)
        .filter_map(|i| raw.get(i..i + 2))
        .collect();
    bytes.join(":")
}
