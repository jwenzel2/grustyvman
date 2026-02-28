use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use quick_xml::events::Event;
use quick_xml::Reader;
use virt::domain::Domain;
use virt::storage_pool::StoragePool;

use crate::backend::backup::{upload_volume_to_pool, unique_vm_name, ProgressFn};
use crate::backend::connection::get_conn;
use crate::backend::types::{DiskFormat, FirmwareType, NetworkSourceType};
use crate::error::AppError;

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct VmwareDisk {
    pub vmdk_path: String,
    pub bus_type: String, // "scsi", "ide", "sata"
}

#[derive(Debug, Clone)]
pub struct VmwareNic {
    pub mac_address: Option<String>,
    pub connection_type: String, // "nat", "bridged", "hostonly", "custom"
    pub adapter_type: String,    // "e1000", "vmxnet3", etc.
}

#[derive(Debug, Clone)]
pub struct VmwareConfig {
    pub name: String,
    pub vcpus: u32,
    pub memory_mib: u64,
    pub firmware: FirmwareType,
    pub disks: Vec<VmwareDisk>,
    pub nics: Vec<VmwareNic>,
    pub guest_os: Option<String>,
    pub source_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ConvertNicConfig {
    pub source_type: NetworkSourceType,
    pub source_value: String,
    pub mac_address: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConvertVmParams {
    pub vmware_config: VmwareConfig,
    pub target_name: String,
    pub disk_format: DiskFormat,
    pub pool_name: String,
    pub nic_configs: Vec<ConvertNicConfig>,
}

// ---------------------------------------------------------------------------
// VMX parser
// ---------------------------------------------------------------------------

pub fn parse_vmx(vmx_path: &str) -> Result<VmwareConfig, AppError> {
    let content = std::fs::read_to_string(vmx_path)?;
    let source_dir = Path::new(vmx_path)
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();

    let mut kv: HashMap<String, String> = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(eq) = line.find('=') {
            let key = line[..eq].trim().to_lowercase();
            let val = line[eq + 1..].trim().trim_matches('"').to_string();
            kv.insert(key, val);
        }
    }

    let name = kv
        .get("displayname")
        .cloned()
        .unwrap_or_else(|| {
            Path::new(vmx_path)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        });

    let vcpus = kv
        .get("numvcpus")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);

    let memory_mib = kv
        .get("memsize")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1024);

    let firmware = if kv.get("firmware").map(|v| v.as_str()) == Some("efi") {
        FirmwareType::Efi
    } else {
        FirmwareType::Bios
    };

    let guest_os = kv.get("guestos").cloned();

    // Discover disks: scsiN:M.fileName, ideN:M.fileName, sataN:M.fileName
    let mut disks = Vec::new();
    for (key, val) in &kv {
        if !key.ends_with(".filename") {
            continue;
        }
        if !val.to_lowercase().ends_with(".vmdk") {
            continue;
        }
        let bus_type = if key.starts_with("scsi") {
            "scsi"
        } else if key.starts_with("ide") {
            "ide"
        } else if key.starts_with("sata") {
            "sata"
        } else {
            continue;
        };

        // Check if .present = TRUE
        let prefix = key.trim_end_matches(".filename");
        let present_key = format!("{prefix}.present");
        let present = kv
            .get(&present_key)
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(true);
        if !present {
            continue;
        }

        let vmdk_path = if Path::new(val).is_absolute() {
            val.clone()
        } else {
            source_dir.join(val).to_string_lossy().to_string()
        };

        disks.push(VmwareDisk {
            vmdk_path,
            bus_type: bus_type.to_string(),
        });
    }

    // Discover NICs: ethernetN.present = TRUE
    let mut nics = Vec::new();
    for n in 0..10 {
        let present_key = format!("ethernet{n}.present");
        let present = kv
            .get(&present_key)
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);
        if !present {
            continue;
        }

        let mac = kv
            .get(&format!("ethernet{n}.generatedaddress"))
            .or_else(|| kv.get(&format!("ethernet{n}.address")))
            .cloned();

        let connection_type = kv
            .get(&format!("ethernet{n}.connectiontype"))
            .cloned()
            .unwrap_or_else(|| "nat".to_string());

        let adapter_type = kv
            .get(&format!("ethernet{n}.virtualdev"))
            .cloned()
            .unwrap_or_else(|| "e1000".to_string());

        nics.push(VmwareNic {
            mac_address: mac,
            connection_type,
            adapter_type,
        });
    }

    Ok(VmwareConfig {
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
// OVF parser
// ---------------------------------------------------------------------------

pub fn parse_ovf(ovf_path: &str) -> Result<VmwareConfig, AppError> {
    let content = std::fs::read_to_string(ovf_path)?;
    let source_dir = Path::new(ovf_path)
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();

    let mut reader = Reader::from_str(&content);
    reader.config_mut().trim_text(true);

    let mut name = String::new();
    let mut vcpus: u32 = 1;
    let mut memory_mib: u64 = 1024;
    let mut firmware = FirmwareType::Bios;
    let guest_os: Option<String> = None;
    let mut disks: Vec<VmwareDisk> = Vec::new();
    let mut nics: Vec<VmwareNic> = Vec::new();

    // Collect file references: id -> href
    let mut file_refs: HashMap<String, String> = HashMap::new();
    // Collect disk references: diskId -> fileRef
    let mut disk_refs: HashMap<String, String> = HashMap::new();

    // Track state for Item parsing
    let mut in_item = false;
    let mut item_resource_type: Option<String> = None;
    let mut item_quantity: Option<String> = None;
    let mut item_connection: Option<String> = None;
    let mut item_resource_sub_type: Option<String> = None;

    // Track current element for text capture
    let mut current_elem = String::new();
    let mut in_virtual_system = false;
    let mut capture_name = false;
    let mut in_operating_system = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local = local_name(e.name().as_ref());
                match local.as_str() {
                    "VirtualSystem" => in_virtual_system = true,
                    "Name" if in_virtual_system && !in_item => capture_name = true,
                    "OperatingSystemSection" => in_operating_system = true,
                    "File" => {
                        let mut file_id = String::new();
                        let mut href = String::new();
                        for attr in e.attributes().flatten() {
                            let aname = local_name(attr.key.as_ref());
                            match aname.as_str() {
                                "id" => file_id = attr_val(&attr),
                                "href" => href = attr_val(&attr),
                                _ => {}
                            }
                        }
                        if !file_id.is_empty() && !href.is_empty() {
                            file_refs.insert(file_id, href);
                        }
                    }
                    "Disk" => {
                        let mut disk_id = String::new();
                        let mut file_ref = String::new();
                        for attr in e.attributes().flatten() {
                            let aname = local_name(attr.key.as_ref());
                            match aname.as_str() {
                                "diskId" => disk_id = attr_val(&attr),
                                "fileRef" => file_ref = attr_val(&attr),
                                _ => {}
                            }
                        }
                        if !disk_id.is_empty() && !file_ref.is_empty() {
                            disk_refs.insert(disk_id, file_ref);
                        }
                    }
                    "Item" | "VirtualHardwareItem" | "StorageItem" | "EthernetPortItem" => {
                        in_item = true;
                        item_resource_type = None;
                        item_quantity = None;
                        item_connection = None;
                        item_resource_sub_type = None;
                    }
                    "ResourceType" if in_item => current_elem = "ResourceType".to_string(),
                    "VirtualQuantity" if in_item => current_elem = "VirtualQuantity".to_string(),
                    "Connection" if in_item => current_elem = "Connection".to_string(),
                    "ResourceSubType" if in_item => current_elem = "ResourceSubType".to_string(),
                    "Config" if in_virtual_system => {
                        // VMware firmware hint: vmw:Config with key="firmware"
                        let mut is_firmware = false;
                        let mut fw_val = String::new();
                        for attr in e.attributes().flatten() {
                            let aname = local_name(attr.key.as_ref());
                            let aval = attr_val(&attr);
                            if aname == "key" && aval == "firmware" {
                                is_firmware = true;
                            }
                            if aname == "value" {
                                fw_val = aval;
                            }
                        }
                        if is_firmware && fw_val == "efi" {
                            firmware = FirmwareType::Efi;
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref t)) => {
                let text = t.unescape().unwrap_or_default().to_string();
                if capture_name && name.is_empty() {
                    name = text.clone();
                }
                if in_operating_system && current_elem.is_empty() {
                    // might capture description
                }
                match current_elem.as_str() {
                    "ResourceType" => item_resource_type = Some(text),
                    "VirtualQuantity" => item_quantity = Some(text),
                    "Connection" => item_connection = Some(text),
                    "ResourceSubType" => item_resource_sub_type = Some(text),
                    _ => {}
                }
                current_elem.clear();
            }
            Ok(Event::End(ref e)) => {
                let local = local_name(e.name().as_ref());
                match local.as_str() {
                    "VirtualSystem" => in_virtual_system = false,
                    "Name" => capture_name = false,
                    "OperatingSystemSection" => in_operating_system = false,
                    "Item" | "VirtualHardwareItem" | "StorageItem" | "EthernetPortItem" => {
                        if in_item {
                            if let Some(ref rt) = item_resource_type {
                                match rt.as_str() {
                                    "3" => {
                                        // Processor
                                        if let Some(ref q) = item_quantity {
                                            if let Ok(v) = q.parse() {
                                                vcpus = v;
                                            }
                                        }
                                    }
                                    "4" => {
                                        // Memory
                                        if let Some(ref q) = item_quantity {
                                            if let Ok(v) = q.parse::<u64>() {
                                                memory_mib = v;
                                            }
                                        }
                                    }
                                    "10" => {
                                        // Ethernet
                                        let adapter = item_resource_sub_type
                                            .clone()
                                            .unwrap_or_else(|| "e1000".to_string());
                                        let conn = item_connection
                                            .clone()
                                            .unwrap_or_else(|| "nat".to_string());
                                        nics.push(VmwareNic {
                                            mac_address: None,
                                            connection_type: conn,
                                            adapter_type: adapter,
                                        });
                                    }
                                    _ => {}
                                }
                            }
                            in_item = false;
                        }
                    }
                    _ => {
                        current_elem.clear();
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    // Resolve disk files
    for (_disk_id, file_ref) in &disk_refs {
        if let Some(href) = file_refs.get(file_ref) {
            let full_path = if Path::new(href).is_absolute() {
                href.clone()
            } else {
                source_dir.join(href).to_string_lossy().to_string()
            };
            disks.push(VmwareDisk {
                vmdk_path: full_path,
                bus_type: "scsi".to_string(),
            });
        }
    }

    // If no disks found via references, scan file_refs directly for .vmdk
    if disks.is_empty() {
        for (_id, href) in &file_refs {
            if href.to_lowercase().ends_with(".vmdk") {
                let full_path = source_dir.join(href).to_string_lossy().to_string();
                disks.push(VmwareDisk {
                    vmdk_path: full_path,
                    bus_type: "scsi".to_string(),
                });
            }
        }
    }

    if name.is_empty() {
        name = Path::new(ovf_path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
    }

    Ok(VmwareConfig {
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
// OVA extractor
// ---------------------------------------------------------------------------

pub fn extract_ova(ova_path: &str) -> Result<(VmwareConfig, PathBuf), AppError> {
    let tmp_dir = tempdir("grustyvman-ova")?;

    let output = std::process::Command::new("tar")
        .args(["xf", ova_path, "-C", &tmp_dir.to_string_lossy()])
        .output()?;
    if !output.status.success() {
        return Err(AppError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!(
                "Failed to extract OVA: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        )));
    }

    // Find .ovf file
    let ovf_path = find_file_with_ext(&tmp_dir, "ovf").ok_or_else(|| {
        AppError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No .ovf file found in OVA archive",
        ))
    })?;

    let config = parse_ovf(&ovf_path.to_string_lossy())?;
    Ok((config, tmp_dir))
}

// ---------------------------------------------------------------------------
// Conversion engine
// ---------------------------------------------------------------------------

pub fn convert_vmware_vm(
    uri: &str,
    params: &ConvertVmParams,
    progress: &ProgressFn,
) -> Result<String, AppError> {
    let num_disks = params.vmware_config.disks.len();
    if num_disks == 0 {
        return Err(AppError::Libvirt(
            "No disks found in VMware configuration".to_string(),
        ));
    }

    progress(0.0, "Preparing conversion...");

    let conn = get_conn(uri)?;
    let final_name = unique_vm_name(&conn, &params.target_name)?;

    // Create temp dir for converted images
    let tmp_dir = tempdir("grustyvman-convert")?;

    // Step 1: Convert VMDK files in parallel via qemu-img
    progress(0.02, "Converting disk images...");
    let format_str = params.disk_format.as_str();
    let ext = params.disk_format.extension();
    let completed = AtomicUsize::new(0);

    let converted_paths: Vec<PathBuf> = std::thread::scope(|s| {
        let handles: Vec<_> = params
            .vmware_config
            .disks
            .iter()
            .enumerate()
            .map(|(i, disk)| {
                let tmp = &tmp_dir;
                let completed = &completed;
                s.spawn(move || {
                    let out_name = format!("disk-{i}.{ext}");
                    let out_path = tmp.join(&out_name);
                    progress(
                        0.02,
                        &format!(
                            "Converting disk {}/{}...",
                            i + 1,
                            num_disks
                        ),
                    );

                    let output = std::process::Command::new("qemu-img")
                        .args([
                            "convert",
                            "-f",
                            "vmdk",
                            "-O",
                            format_str,
                            &disk.vmdk_path,
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
                    progress(
                        frac,
                        &format!("Converted disk {done}/{num_disks}"),
                    );
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
                    let path =
                        upload_volume_to_pool(&conn, &pool, src, vol_name, |_| {})?;
                    let done = uploaded_completed.fetch_add(1, Ordering::Relaxed) + 1;
                    let frac = 0.45 + 0.40 * (done as f64 / num_disks as f64);
                    progress(
                        frac,
                        &format!("Uploaded disk {done}/{num_disks}"),
                    );
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
    let xml = generate_converted_domain_xml(&final_name, params, &disk_paths);
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
// Domain XML generation for converted VMs
// ---------------------------------------------------------------------------

fn generate_converted_domain_xml(
    name: &str,
    params: &ConvertVmParams,
    disk_paths: &[String],
) -> String {
    let memory_kib = params.vmware_config.memory_mib * 1024;
    let vcpus = params.vmware_config.vcpus;

    let os_tag = match params.vmware_config.firmware {
        FirmwareType::Efi => r#"<os firmware="efi">"#,
        FirmwareType::Bios => "<os>",
    };

    let smm_feature = match params.vmware_config.firmware {
        FirmwareType::Efi => "\n    <smm state=\"on\"/>",
        FirmwareType::Bios => "",
    };

    // Generate disk XML entries
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

    // Generate NIC XML entries
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

fn tempdir(prefix: &str) -> Result<PathBuf, std::io::Error> {
    let name = format!("{prefix}-{}", std::process::id());
    let path = PathBuf::from("/var/tmp").join(name);
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

fn find_file_with_ext(dir: &Path, ext: &str) -> Option<PathBuf> {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some(ext) {
                return Some(path);
            }
            // Check one level deep
            if path.is_dir() {
                if let Some(found) = find_file_with_ext(&path, ext) {
                    return Some(found);
                }
            }
        }
    }
    None
}

/// Extract local name from a possibly-namespaced XML tag (strip namespace prefix).
fn local_name(raw: &[u8]) -> String {
    let full = String::from_utf8_lossy(raw).to_string();
    if let Some(pos) = full.rfind(':') {
        full[pos + 1..].to_string()
    } else {
        full
    }
}

fn attr_val(attr: &quick_xml::events::attributes::Attribute) -> String {
    String::from_utf8_lossy(&attr.value).to_string()
}
