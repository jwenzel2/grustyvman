use virt::domain::Domain;
use crate::backend::connection::get_conn;

use crate::backend::types::RawPerfSample;
use crate::error::AppError;

pub fn collect_perf_sample(
    uri: &str,
    uuid: &str,
    disk_targets: &[String],
    iface_targets: &[String],
) -> Result<RawPerfSample, AppError> {
    let conn = get_conn(uri)?;
    let domain = Domain::lookup_by_uuid_string(&conn, uuid)?;

    let info = domain.get_info()?;
    let cpu_time_ns = info.cpu_time;
    let nr_vcpus = info.nr_virt_cpu;
    let memory_total_kib = info.memory;

    // Get memory stats - tag 4 is VIR_DOMAIN_MEMORY_STAT_UNUSED
    let memory_unused_kib = match domain.memory_stats(0) {
        Ok(stats) => stats
            .iter()
            .find(|s| s.tag == 4)
            .map(|s| s.val)
            .unwrap_or(0),
        Err(_) => 0,
    };

    // Sum disk I/O across all disk targets
    let mut disk_rd_bytes: i64 = 0;
    let mut disk_wr_bytes: i64 = 0;
    for dev in disk_targets {
        if let Ok(stats) = domain.get_block_stats(dev) {
            disk_rd_bytes += stats.rd_bytes;
            disk_wr_bytes += stats.wr_bytes;
        }
    }

    // Interface target names (vnetX) can change on power-cycle/reconnect.
    // Prefer live targets from current runtime XML, then fall back to cached.
    let live_iface_targets = domain
        .get_xml_desc(0)
        .ok()
        .map(|xml| crate::backend::domain_xml::extract_interface_targets(&xml))
        .filter(|targets| !targets.is_empty());
    let iface_targets = live_iface_targets.as_deref().unwrap_or(iface_targets);

    // Sum network I/O across all interface targets
    let mut net_rx_bytes: i64 = 0;
    let mut net_tx_bytes: i64 = 0;
    for iface in iface_targets {
        if let Ok(stats) = domain.interface_stats(iface) {
            net_rx_bytes += stats.rx_bytes;
            net_tx_bytes += stats.tx_bytes;
        }
    }

    let timestamp_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;

    Ok(RawPerfSample {
        timestamp_ns,
        cpu_time_ns,
        nr_vcpus,
        memory_total_kib,
        memory_unused_kib,
        disk_rd_bytes,
        disk_wr_bytes,
        net_rx_bytes,
        net_tx_bytes,
    })
}
