use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::backend::types::HostdevInfo;

fn read_sysfs_attr(path: &Path, attr: &str) -> Option<String> {
    fs::read_to_string(path.join(attr))
        .ok()
        .map(|s| s.trim().to_string())
}

/// Parse /usr/share/hwdata/pci.ids into (vendor_id -> (vendor_name, device_id -> device_name)).
fn load_pci_ids() -> HashMap<String, (String, HashMap<String, String>)> {
    let mut map: HashMap<String, (String, HashMap<String, String>)> = HashMap::new();
    let paths = ["/usr/share/hwdata/pci.ids", "/usr/share/misc/pci.ids"];
    let content = paths.iter().find_map(|p| fs::read_to_string(p).ok());
    let content = match content {
        Some(c) => c,
        None => return map,
    };

    let mut current_vendor: Option<String> = None;
    for line in content.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        // Subsystem line (two tabs) — skip
        if line.starts_with("\t\t") {
            continue;
        }
        // Device line (one tab)
        if line.starts_with('\t') {
            if let Some(vid) = &current_vendor {
                let trimmed = &line[1..];
                if let Some((did, name)) = trimmed.split_once("  ") {
                    let did = did.trim().to_lowercase();
                    let name = name.trim().to_string();
                    if let Some(entry) = map.get_mut(vid) {
                        entry.1.insert(did, name);
                    }
                }
            }
            continue;
        }
        // Vendor line
        if let Some((vid, name)) = line.split_once("  ") {
            let vid = vid.trim().to_lowercase();
            let name = name.trim().to_string();
            current_vendor = Some(vid.clone());
            map.insert(vid, (name, HashMap::new()));
        }
    }
    map
}

fn pci_display_name(
    address: &str,
    vendor_hex: &str,
    device_hex: &str,
    pci_ids: &HashMap<String, (String, HashMap<String, String>)>,
) -> String {
    // sysfs gives "0x10de"; strip prefix and lowercase for lookup
    let vid = vendor_hex.trim_start_matches("0x").to_lowercase();
    let did = device_hex.trim_start_matches("0x").to_lowercase();

    let vendor_name = pci_ids.get(&vid).map(|(n, _)| n.as_str()).unwrap_or("");
    let device_name = pci_ids
        .get(&vid)
        .and_then(|(_, devs)| devs.get(&did))
        .map(|s| s.as_str())
        .unwrap_or("");

    match (vendor_name.is_empty(), device_name.is_empty()) {
        (false, false) => format!("{} {} [{}]", vendor_name, device_name, address),
        (false, true) => format!("{} {} [{}]", vendor_name, did, address),
        _ => format!("{} {} [{}]", vid, did, address),
    }
}

/// Enumerate host PCI devices from /sys/bus/pci/devices.
/// Skips PCI bridges and host bridges (class 0x060000–0x0607ff).
pub fn list_pci_devices() -> Vec<HostdevInfo> {
    let pci_dir = Path::new("/sys/bus/pci/devices");
    let mut devices = Vec::new();
    let pci_ids = load_pci_ids();

    let entries = match fs::read_dir(pci_dir) {
        Ok(e) => e,
        Err(_) => return devices,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        // name format: "0000:00:1f.2"
        let parts: Vec<&str> = name.splitn(2, ':').collect();
        if parts.len() != 2 {
            continue;
        }
        let domain_hex = parts[0];
        let rest = parts[1];
        let parts2: Vec<&str> = rest.splitn(2, ':').collect();
        if parts2.len() != 2 {
            continue;
        }
        let bus_hex = parts2[0];
        let sf = parts2[1];
        let parts3: Vec<&str> = sf.splitn(2, '.').collect();
        if parts3.len() != 2 {
            continue;
        }
        let slot_hex = parts3[0];
        let func_hex = parts3[1];

        // Skip PCI bridges (class starts with 0x0604 or 0x0600)
        if let Some(class) = read_sysfs_attr(&path, "class") {
            let class_low = class.to_lowercase();
            if class_low.starts_with("0x0604") || class_low.starts_with("0x0600") {
                continue;
            }
        }

        let vendor = read_sysfs_attr(&path, "vendor").unwrap_or_default();
        let device_id = read_sysfs_attr(&path, "device").unwrap_or_default();

        let display_name = pci_display_name(&name, &vendor, &device_id, &pci_ids);

        devices.push(HostdevInfo {
            device_type: "pci".to_string(),
            pci_domain: Some(format!("0x{:04}", u64::from_str_radix(domain_hex, 16).unwrap_or(0))),
            pci_bus: Some(format!("0x{:02x}", u64::from_str_radix(bus_hex, 16).unwrap_or(0))),
            pci_slot: Some(format!("0x{:02x}", u64::from_str_radix(slot_hex, 16).unwrap_or(0))),
            pci_function: Some(format!("0x{}", u64::from_str_radix(func_hex, 16).unwrap_or(0))),
            usb_vendor: None,
            usb_product: None,
            display_name,
        });
    }

    devices.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    devices
}

/// Enumerate host USB devices from /sys/bus/usb/devices.
/// Skips root hubs (idVendor == 1d6b, Linux Foundation).
pub fn list_usb_devices() -> Vec<HostdevInfo> {
    let usb_dir = Path::new("/sys/bus/usb/devices");
    let mut devices = Vec::new();

    let entries = match fs::read_dir(usb_dir) {
        Ok(e) => e,
        Err(_) => return devices,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        let vendor_id = match read_sysfs_attr(&path, "idVendor") {
            Some(v) => v,
            None => continue,
        };
        let product_id = match read_sysfs_attr(&path, "idProduct") {
            Some(p) => p,
            None => continue,
        };

        // Skip Linux Foundation virtual hubs
        if vendor_id == "1d6b" {
            continue;
        }

        let manufacturer = read_sysfs_attr(&path, "manufacturer").unwrap_or_default();
        let product = read_sysfs_attr(&path, "product").unwrap_or_default();

        let display_name = if !manufacturer.is_empty() || !product.is_empty() {
            format!("{} {} [{}:{}]", manufacturer, product, vendor_id, product_id)
                .trim()
                .to_string()
        } else {
            format!("USB Device [{}:{}]", vendor_id, product_id)
        };

        devices.push(HostdevInfo {
            device_type: "usb".to_string(),
            pci_domain: None,
            pci_bus: None,
            pci_slot: None,
            pci_function: None,
            usb_vendor: Some(format!("0x{}", vendor_id)),
            usb_product: Some(format!("0x{}", product_id)),
            display_name,
        });
    }

    devices.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    devices
}
