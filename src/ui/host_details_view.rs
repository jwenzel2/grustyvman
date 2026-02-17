use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use crate::backend::types::HostInfo;

pub struct HostDetailsView {
    pub container: gtk::Box,
    hostname_row: adw::ActionRow,
    uri_row: adw::ActionRow,
    libvirt_version_row: adw::ActionRow,
    hypervisor_version_row: adw::ActionRow,
    cpu_model_row: adw::ActionRow,
    cpu_topology_row: adw::ActionRow,
    cpu_mhz_row: adw::ActionRow,
    memory_row: adw::ActionRow,
}

impl HostDetailsView {
    pub fn new() -> Self {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 24);
        container.set_margin_top(24);
        container.set_margin_bottom(24);
        container.set_margin_start(24);
        container.set_margin_end(24);

        // Connection group
        let conn_group = adw::PreferencesGroup::new();
        conn_group.set_title("Connection");

        let hostname_row = adw::ActionRow::new();
        hostname_row.set_title("Hostname");
        hostname_row.set_subtitle("...");
        hostname_row.set_activatable(false);
        conn_group.add(&hostname_row);

        let uri_row = adw::ActionRow::new();
        uri_row.set_title("URI");
        uri_row.set_subtitle("...");
        uri_row.set_activatable(false);
        conn_group.add(&uri_row);

        let libvirt_version_row = adw::ActionRow::new();
        libvirt_version_row.set_title("Libvirt Version");
        libvirt_version_row.set_subtitle("...");
        libvirt_version_row.set_activatable(false);
        conn_group.add(&libvirt_version_row);

        let hypervisor_version_row = adw::ActionRow::new();
        hypervisor_version_row.set_title("Hypervisor Version");
        hypervisor_version_row.set_subtitle("...");
        hypervisor_version_row.set_activatable(false);
        conn_group.add(&hypervisor_version_row);

        container.append(&conn_group);

        // CPU group
        let cpu_group = adw::PreferencesGroup::new();
        cpu_group.set_title("CPU");

        let cpu_model_row = adw::ActionRow::new();
        cpu_model_row.set_title("Model");
        cpu_model_row.set_subtitle("...");
        cpu_model_row.set_activatable(false);
        cpu_group.add(&cpu_model_row);

        let cpu_topology_row = adw::ActionRow::new();
        cpu_topology_row.set_title("Topology");
        cpu_topology_row.set_subtitle("...");
        cpu_topology_row.set_activatable(false);
        cpu_group.add(&cpu_topology_row);

        let cpu_mhz_row = adw::ActionRow::new();
        cpu_mhz_row.set_title("Frequency");
        cpu_mhz_row.set_subtitle("...");
        cpu_mhz_row.set_activatable(false);
        cpu_group.add(&cpu_mhz_row);

        container.append(&cpu_group);

        // Memory group
        let mem_group = adw::PreferencesGroup::new();
        mem_group.set_title("Memory");

        let memory_row = adw::ActionRow::new();
        memory_row.set_title("Total RAM");
        memory_row.set_subtitle("...");
        memory_row.set_activatable(false);
        mem_group.add(&memory_row);

        container.append(&mem_group);

        Self {
            container,
            hostname_row,
            uri_row,
            libvirt_version_row,
            hypervisor_version_row,
            cpu_model_row,
            cpu_topology_row,
            cpu_mhz_row,
            memory_row,
        }
    }

    pub fn update(&self, info: &HostInfo) {
        self.hostname_row.set_subtitle(&info.hostname);
        self.uri_row.set_subtitle(&info.uri);
        self.libvirt_version_row.set_subtitle(&info.libvirt_version);
        self.hypervisor_version_row.set_subtitle(&info.hypervisor_version);
        self.cpu_model_row.set_subtitle(&info.cpu_model);

        let total_threads = info.cpu_sockets * info.cpu_cores * info.cpu_threads;
        let topology = format!(
            "{} socket(s), {} core(s), {} thread(s) = {} logical CPUs ({} NUMA node(s))",
            info.cpu_sockets, info.cpu_cores, info.cpu_threads, total_threads, info.cpu_nodes
        );
        self.cpu_topology_row.set_subtitle(&topology);

        self.cpu_mhz_row.set_subtitle(&format!("{} MHz", info.cpu_mhz));

        let mem_gib = info.memory_kib as f64 / (1024.0 * 1024.0);
        self.memory_row.set_subtitle(&format!("{:.1} GiB ({} KiB)", mem_gib, info.memory_kib));
    }
}
