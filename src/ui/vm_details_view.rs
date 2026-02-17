use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use crate::backend::types::DomainDetails;

pub struct VmDetailsView {
    pub container: gtk::Box,
    status_row: adw::ActionRow,
    id_row: adw::ActionRow,
    uuid_row: adw::ActionRow,
    autostart_row: adw::ActionRow,
    vcpus_row: adw::ActionRow,
    memory_row: adw::ActionRow,
    os_row: adw::ActionRow,
    firmware_row: adw::ActionRow,
    cpu_mode_row: adw::ActionRow,
    boot_group: adw::PreferencesGroup,
    disks_group: adw::PreferencesGroup,
    networks_group: adw::PreferencesGroup,
    display_group: adw::PreferencesGroup,
    tpm_row: adw::ActionRow,
    filesystems_group: adw::PreferencesGroup,
    cpu_pinning_group: adw::PreferencesGroup,
}

impl VmDetailsView {
    pub fn new() -> Self {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 24);
        container.set_margin_top(24);
        container.set_margin_bottom(24);
        container.set_margin_start(24);
        container.set_margin_end(24);

        // Status group
        let status_group = adw::PreferencesGroup::new();
        status_group.set_title("Status");

        let status_row = adw::ActionRow::new();
        status_row.set_title("State");
        status_row.set_activatable(false);
        status_group.add(&status_row);

        let id_row = adw::ActionRow::new();
        id_row.set_title("Domain ID");
        id_row.set_activatable(false);
        status_group.add(&id_row);

        let uuid_row = adw::ActionRow::new();
        uuid_row.set_title("UUID");
        uuid_row.set_activatable(false);
        status_group.add(&uuid_row);

        let autostart_row = adw::ActionRow::new();
        autostart_row.set_title("Autostart");
        autostart_row.set_activatable(false);
        status_group.add(&autostart_row);

        container.append(&status_group);

        // Resources group
        let resources_group = adw::PreferencesGroup::new();
        resources_group.set_title("Resources");

        let vcpus_row = adw::ActionRow::new();
        vcpus_row.set_title("vCPUs");
        vcpus_row.set_activatable(false);
        resources_group.add(&vcpus_row);

        let memory_row = adw::ActionRow::new();
        memory_row.set_title("Memory");
        memory_row.set_activatable(false);
        resources_group.add(&memory_row);

        let os_row = adw::ActionRow::new();
        os_row.set_title("OS Type");
        os_row.set_activatable(false);
        resources_group.add(&os_row);

        let firmware_row = adw::ActionRow::new();
        firmware_row.set_title("Firmware");
        firmware_row.set_activatable(false);
        resources_group.add(&firmware_row);

        let cpu_mode_row = adw::ActionRow::new();
        cpu_mode_row.set_title("CPU Mode");
        cpu_mode_row.set_activatable(false);
        resources_group.add(&cpu_mode_row);

        container.append(&resources_group);

        // Boot order group
        let boot_group = adw::PreferencesGroup::new();
        boot_group.set_title("Boot Order");
        container.append(&boot_group);

        // Storage group
        let disks_group = adw::PreferencesGroup::new();
        disks_group.set_title("Storage");
        container.append(&disks_group);

        // Network group
        let networks_group = adw::PreferencesGroup::new();
        networks_group.set_title("Network");
        container.append(&networks_group);

        // Display &amp; Media group
        let display_group = adw::PreferencesGroup::new();
        display_group.set_title("Display &amp; Media");

        let tpm_row = adw::ActionRow::new();
        tpm_row.set_title("TPM");
        tpm_row.set_activatable(false);
        display_group.add(&tpm_row);

        container.append(&display_group);

        // Shared Folders group
        let filesystems_group = adw::PreferencesGroup::new();
        filesystems_group.set_title("Shared Folders");
        container.append(&filesystems_group);

        // CPU Pinning group
        let cpu_pinning_group = adw::PreferencesGroup::new();
        cpu_pinning_group.set_title("CPU Pinning");
        container.append(&cpu_pinning_group);

        Self {
            container,
            status_row,
            id_row,
            uuid_row,
            autostart_row,
            vcpus_row,
            memory_row,
            os_row,
            firmware_row,
            cpu_mode_row,
            boot_group,
            disks_group,
            networks_group,
            display_group,
            tpm_row,
            filesystems_group,
            cpu_pinning_group,
        }
    }

    pub fn update(&self, details: &DomainDetails, state_label: &str, domain_id: Option<u32>, autostart: bool) {
        self.status_row.set_subtitle(state_label);
        self.id_row.set_subtitle(&domain_id.map(|id| id.to_string()).unwrap_or_else(|| "-".to_string()));
        self.uuid_row.set_subtitle(&details.uuid);
        self.autostart_row.set_subtitle(if autostart { "Yes" } else { "No" });
        self.vcpus_row.set_subtitle(&details.vcpus.to_string());
        self.memory_row.set_subtitle(&format!("{} MiB", details.memory_kib / 1024));
        self.os_row.set_subtitle(&format!("{} ({})", details.os_type, details.arch));
        self.firmware_row.set_subtitle(details.firmware.label());

        // CPU mode
        let cpu_subtitle = match details.cpu_mode {
            crate::backend::types::CpuMode::Custom => {
                if let Some(ref model) = details.cpu_model {
                    format!("{} ({})", details.cpu_mode.label(), model)
                } else {
                    details.cpu_mode.label().to_string()
                }
            }
            _ => details.cpu_mode.label().to_string(),
        };
        self.cpu_mode_row.set_subtitle(&cpu_subtitle);

        // Boot order
        clear_pref_group(&self.boot_group);
        if details.boot_order.is_empty() {
            let row = adw::ActionRow::new();
            row.set_title("No boot order defined");
            row.set_activatable(false);
            self.boot_group.add(&row);
        } else {
            for (i, dev) in details.boot_order.iter().enumerate() {
                let row = adw::ActionRow::new();
                row.set_title(&format!("{}. {}", i + 1, dev.label()));
                row.set_activatable(false);
                self.boot_group.add(&row);
            }
        }

        // Clear and rebuild disks
        clear_pref_group(&self.disks_group);
        if details.disks.is_empty() {
            let row = adw::ActionRow::new();
            row.set_title("No disks");
            row.set_activatable(false);
            self.disks_group.add(&row);
        } else {
            for disk in &details.disks {
                let row = adw::ActionRow::new();
                let type_label = if disk.device_type == "cdrom" { " (CD-ROM)" } else { "" };
                row.set_title(&format!("/dev/{}{}", disk.target_dev, type_label));
                row.set_subtitle(&disk.source_file.clone().unwrap_or_else(|| "No source".to_string()));
                row.set_activatable(false);
                self.disks_group.add(&row);
            }
        }

        // Clear and rebuild networks
        clear_pref_group(&self.networks_group);
        if details.networks.is_empty() {
            let row = adw::ActionRow::new();
            row.set_title("No network interfaces");
            row.set_activatable(false);
            self.networks_group.add(&row);
        } else {
            for net in &details.networks {
                let row = adw::ActionRow::new();
                let title = net.mac_address.clone().unwrap_or_else(|| "Unknown MAC".to_string());
                row.set_title(&title);
                let subtitle = format!(
                    "Network: {} | Model: {}",
                    net.source_network.as_deref().unwrap_or("N/A"),
                    net.model_type.as_deref().unwrap_or("N/A")
                );
                row.set_subtitle(&subtitle);
                row.set_activatable(false);
                self.networks_group.add(&row);
            }
        }

        // Display &amp; Media
        clear_pref_group(&self.display_group);

        if let Some(ref gfx) = details.graphics {
            let row = adw::ActionRow::new();
            row.set_title("Graphics");
            let port_str = gfx.port.map(|p| format!(" (port {})", p)).unwrap_or_default();
            row.set_subtitle(&format!("{}{}", gfx.graphics_type.label(), port_str));
            row.set_activatable(false);
            self.display_group.add(&row);
        } else {
            let row = adw::ActionRow::new();
            row.set_title("Graphics");
            row.set_subtitle("None");
            row.set_activatable(false);
            self.display_group.add(&row);
        }

        if let Some(ref vid) = details.video {
            let row = adw::ActionRow::new();
            row.set_title("Video");
            row.set_subtitle(vid.model.label());
            row.set_activatable(false);
            self.display_group.add(&row);
        } else {
            let row = adw::ActionRow::new();
            row.set_title("Video");
            row.set_subtitle("None");
            row.set_activatable(false);
            self.display_group.add(&row);
        }

        if let Some(ref snd) = details.sound {
            let row = adw::ActionRow::new();
            row.set_title("Sound");
            row.set_subtitle(snd.model.label());
            row.set_activatable(false);
            self.display_group.add(&row);
        } else {
            let row = adw::ActionRow::new();
            row.set_title("Sound");
            row.set_subtitle("None");
            row.set_activatable(false);
            self.display_group.add(&row);
        }

        // TPM
        if let Some(ref tpm) = details.tpm {
            self.tpm_row.set_subtitle(&format!("TPM {} ({})", tpm.version, tpm.model.label()));
        } else {
            self.tpm_row.set_subtitle("None");
        }

        // Shared Folders
        clear_pref_group(&self.filesystems_group);
        if details.filesystems.is_empty() {
            let row = adw::ActionRow::new();
            row.set_title("No shared folders");
            row.set_activatable(false);
            self.filesystems_group.add(&row);
        } else {
            for fs in &details.filesystems {
                let row = adw::ActionRow::new();
                row.set_title(&fs.target_dir);
                row.set_subtitle(&format!("{} ({})", fs.source_dir, fs.driver));
                row.set_activatable(false);
                self.filesystems_group.add(&row);
            }
        }

        // CPU Pinning
        clear_pref_group(&self.cpu_pinning_group);
        if details.cpu_tune.vcpu_pins.is_empty() && details.cpu_tune.emulatorpin.is_none() {
            let row = adw::ActionRow::new();
            row.set_title("No CPU pinning configured");
            row.set_activatable(false);
            self.cpu_pinning_group.add(&row);
        } else {
            for pin in &details.cpu_tune.vcpu_pins {
                let row = adw::ActionRow::new();
                row.set_title(&format!("vCPU {}", pin.vcpu));
                row.set_subtitle(&format!("→ cores {}", pin.cpuset));
                row.set_activatable(false);
                self.cpu_pinning_group.add(&row);
            }
            if let Some(ref cpuset) = details.cpu_tune.emulatorpin {
                let row = adw::ActionRow::new();
                row.set_title("Emulator Thread");
                row.set_subtitle(&format!("→ cores {cpuset}"));
                row.set_activatable(false);
                self.cpu_pinning_group.add(&row);
            }
        }
    }
}

fn clear_pref_group(group: &adw::PreferencesGroup) {
    let mut rows_to_remove = Vec::new();
    let mut child = group.first_child();
    while let Some(c) = child {
        let next = c.next_sibling();
        if c.downcast_ref::<adw::ActionRow>().is_some() {
            rows_to_remove.push(c);
        }
        child = next;
    }
    for row in rows_to_remove {
        group.remove(&row);
    }
}
