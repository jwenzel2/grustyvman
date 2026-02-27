use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;

use crate::backend::types::PerfDataPoint;
use crate::ui::perf_graph::PerfGraph;

pub struct VmPerformanceView {
    pub container: gtk::Box,
    cpu_graph: PerfGraph,
    mem_graph: PerfGraph,
    disk_graph: PerfGraph,
    net_graph: PerfGraph,
    cpu_detail: adw::ActionRow,
    mem_detail: adw::ActionRow,
    disk_detail: adw::ActionRow,
    net_detail: adw::ActionRow,
}

impl VmPerformanceView {
    pub fn new() -> Self {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 24);
        container.set_margin_top(24);
        container.set_margin_bottom(24);
        container.set_margin_start(24);
        container.set_margin_end(24);

        // CPU graph - green, fixed max 100%
        let cpu_graph = PerfGraph::new(
            "CPU Usage",
            "%",
            (0.18, 0.76, 0.49), // #2ec27e
            60,
            Some(100.0),
        );
        let cpu_group = adw::PreferencesGroup::new();
        cpu_group.set_title("CPU");
        cpu_group.add(&cpu_graph.widget);
        let cpu_detail = adw::ActionRow::new();
        cpu_detail.set_title("Usage");
        cpu_detail.set_subtitle("--");
        cpu_detail.set_activatable(false);
        cpu_group.add(&cpu_detail);
        container.append(&cpu_group);

        // Memory graph - blue, fixed max 100%
        let mem_graph = PerfGraph::new(
            "Memory Usage",
            "%",
            (0.24, 0.56, 0.96), // #3d8ff8
            60,
            Some(100.0),
        );
        let mem_group = adw::PreferencesGroup::new();
        mem_group.set_title("Memory");
        mem_group.add(&mem_graph.widget);
        let mem_detail = adw::ActionRow::new();
        mem_detail.set_title("Usage");
        mem_detail.set_subtitle("--");
        mem_detail.set_activatable(false);
        mem_group.add(&mem_detail);
        container.append(&mem_group);

        // Disk I/O graph - orange, auto-scale
        let disk_graph = PerfGraph::new(
            "Disk I/O",
            "B/s",
            (0.96, 0.47, 0.0), // #f57800
            60,
            None,
        );
        let disk_group = adw::PreferencesGroup::new();
        disk_group.set_title("Disk I/O");
        disk_group.add(&disk_graph.widget);
        let disk_detail = adw::ActionRow::new();
        disk_detail.set_title("Read / Write");
        disk_detail.set_subtitle("--");
        disk_detail.set_activatable(false);
        disk_group.add(&disk_detail);
        container.append(&disk_group);

        // Network I/O graph - purple, auto-scale
        let net_graph = PerfGraph::new(
            "Network I/O",
            "B/s",
            (0.57, 0.36, 0.82), // #925cd2
            60,
            None,
        );
        let net_group = adw::PreferencesGroup::new();
        net_group.set_title("Network I/O");
        net_group.add(&net_graph.widget);
        let net_detail = adw::ActionRow::new();
        net_detail.set_title("RX / TX");
        net_detail.set_subtitle("--");
        net_detail.set_activatable(false);
        net_group.add(&net_detail);
        container.append(&net_group);

        Self {
            container,
            cpu_graph,
            mem_graph,
            disk_graph,
            net_graph,
            cpu_detail,
            mem_detail,
            disk_detail,
            net_detail,
        }
    }

    pub fn update(&self, point: &PerfDataPoint) {
        self.cpu_graph.push_value(point.cpu_percent);
        self.mem_graph.push_value(point.memory_used_percent);
        self.disk_graph.push_value(point.disk_read_bytes_sec + point.disk_write_bytes_sec);
        self.net_graph.push_value(point.net_rx_bytes_sec + point.net_tx_bytes_sec);

        self.cpu_detail.set_subtitle(&format!("{:.1}%", point.cpu_percent));
        self.mem_detail.set_subtitle(&format!(
            "{:.0} / {:.0} MiB ({:.1}%)",
            point.memory_used_mib, point.memory_total_mib, point.memory_used_percent
        ));
        self.disk_detail.set_subtitle(&format!(
            "R: {} / W: {}",
            format_rate(point.disk_read_bytes_sec),
            format_rate(point.disk_write_bytes_sec)
        ));
        self.net_detail.set_subtitle(&format!(
            "RX: {} / TX: {}",
            format_rate(point.net_rx_bytes_sec),
            format_rate(point.net_tx_bytes_sec)
        ));
    }

    pub fn clear(&self) {
        self.cpu_graph.clear();
        self.mem_graph.clear();
        self.disk_graph.clear();
        self.net_graph.clear();
        self.cpu_detail.set_subtitle("--");
        self.mem_detail.set_subtitle("--");
        self.disk_detail.set_subtitle("--");
        self.net_detail.set_subtitle("--");
    }
}

fn format_rate(bytes_sec: f64) -> String {
    if bytes_sec >= 1_073_741_824.0 {
        format!("{:.1} GB/s", bytes_sec / 1_073_741_824.0)
    } else if bytes_sec >= 1_048_576.0 {
        format!("{:.1} MB/s", bytes_sec / 1_048_576.0)
    } else if bytes_sec >= 1024.0 {
        format!("{:.1} KB/s", bytes_sec / 1024.0)
    } else {
        format!("{:.0} B/s", bytes_sec)
    }
}
