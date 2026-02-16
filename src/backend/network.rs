use virt::connect::Connect;
use virt::network::Network;

use crate::backend::types::{
    ForwardMode, NetworkCreateParams, NetworkState, VirtNetworkInfo,
};
use crate::error::AppError;

fn with_network<F, R>(uri: &str, uuid: &str, f: F) -> Result<R, AppError>
where
    F: FnOnce(&Network) -> Result<R, AppError>,
{
    let conn = Connect::open(Some(uri))?;
    let network = Network::lookup_by_uuid_string(&conn, uuid)?;
    f(&network)
}

pub fn list_all_networks(uri: &str) -> Result<Vec<VirtNetworkInfo>, AppError> {
    let conn = Connect::open(Some(uri))?;
    let networks = conn.list_all_networks(0)?;

    let mut result = Vec::new();
    for net in &networks {
        let name = net.get_name()?;
        let uuid = net.get_uuid_string()?;
        let active = net.is_active().unwrap_or(false);
        let persistent = net.is_persistent().unwrap_or(false);
        let autostart = net.get_autostart().unwrap_or(false);

        let state = if active {
            NetworkState::Active
        } else {
            NetworkState::Inactive
        };

        let xml = net.get_xml_desc(0).unwrap_or_default();
        let parsed = parse_network_xml(&xml);

        result.push(VirtNetworkInfo {
            name,
            uuid,
            state,
            active,
            persistent,
            autostart,
            forward_mode: parsed.forward_mode,
            bridge_name: parsed.bridge_name,
            ip_address: parsed.ip_address,
            ip_netmask: parsed.ip_netmask,
            dhcp_start: parsed.dhcp_start,
            dhcp_end: parsed.dhcp_end,
        });
    }

    result.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(result)
}

pub fn start_network(uri: &str, uuid: &str) -> Result<(), AppError> {
    with_network(uri, uuid, |network| {
        let _ = network.create();
        Ok(())
    })
}

pub fn stop_network(uri: &str, uuid: &str) -> Result<(), AppError> {
    with_network(uri, uuid, |network| {
        network.destroy()?;
        Ok(())
    })
}

pub fn delete_network(uri: &str, uuid: &str) -> Result<(), AppError> {
    with_network(uri, uuid, |network| {
        let _ = network.destroy();
        network.undefine()?;
        Ok(())
    })
}

pub fn set_network_autostart(uri: &str, uuid: &str, autostart: bool) -> Result<(), AppError> {
    with_network(uri, uuid, |network| {
        let _ = network.set_autostart(autostart);
        Ok(())
    })
}

pub fn create_network(uri: &str, params: &NetworkCreateParams) -> Result<(), AppError> {
    let xml = build_network_xml(params);
    let conn = Connect::open(Some(uri))?;
    let network = Network::define_xml(&conn, &xml)?;
    let _ = network.create();
    Ok(())
}

struct ParsedNetwork {
    forward_mode: ForwardMode,
    bridge_name: Option<String>,
    ip_address: Option<String>,
    ip_netmask: Option<String>,
    dhcp_start: Option<String>,
    dhcp_end: Option<String>,
}

fn parse_network_xml(xml: &str) -> ParsedNetwork {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut forward_mode = None;
    let mut bridge_name = None;
    let mut ip_address = None;
    let mut ip_netmask = None;
    let mut dhcp_start = None;
    let mut dhcp_end = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "forward" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"mode" {
                                let mode_str =
                                    String::from_utf8_lossy(&attr.value).to_string();
                                forward_mode = Some(ForwardMode::from_str(&mode_str));
                            }
                        }
                    }
                    "bridge" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"name" {
                                bridge_name = Some(
                                    String::from_utf8_lossy(&attr.value).to_string(),
                                );
                            }
                        }
                    }
                    "ip" => {
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"address" => {
                                    ip_address = Some(
                                        String::from_utf8_lossy(&attr.value).to_string(),
                                    );
                                }
                                b"netmask" => {
                                    ip_netmask = Some(
                                        String::from_utf8_lossy(&attr.value).to_string(),
                                    );
                                }
                                _ => {}
                            }
                        }
                    }
                    "range" => {
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"start" => {
                                    dhcp_start = Some(
                                        String::from_utf8_lossy(&attr.value).to_string(),
                                    );
                                }
                                b"end" => {
                                    dhcp_end = Some(
                                        String::from_utf8_lossy(&attr.value).to_string(),
                                    );
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    ParsedNetwork {
        forward_mode: forward_mode.unwrap_or(ForwardMode::Isolated),
        bridge_name,
        ip_address,
        ip_netmask,
        dhcp_start,
        dhcp_end,
    }
}

fn build_network_xml(params: &NetworkCreateParams) -> String {
    let mut xml = format!("<network>\n  <name>{}</name>\n", params.name);

    match params.forward_mode {
        ForwardMode::Isolated => {
            // No forward element for isolated networks
        }
        mode => {
            xml.push_str(&format!("  <forward mode=\"{}\"/>\n", mode.as_str()));
        }
    }

    if params.forward_mode == ForwardMode::Bridge && !params.bridge_name.is_empty() {
        xml.push_str(&format!(
            "  <bridge name=\"{}\"/>\n",
            params.bridge_name
        ));
    } else if params.forward_mode != ForwardMode::Bridge {
        xml.push_str("  <bridge stp=\"on\" delay=\"0\"/>\n");
    }

    if params.forward_mode != ForwardMode::Bridge && !params.ip_address.is_empty() {
        xml.push_str(&format!(
            "  <ip address=\"{}\" netmask=\"{}\">\n",
            params.ip_address, params.ip_netmask
        ));

        if params.dhcp_enabled && !params.dhcp_start.is_empty() && !params.dhcp_end.is_empty() {
            xml.push_str("    <dhcp>\n");
            xml.push_str(&format!(
                "      <range start=\"{}\" end=\"{}\"/>\n",
                params.dhcp_start, params.dhcp_end
            ));
            xml.push_str("    </dhcp>\n");
        }

        xml.push_str("  </ip>\n");
    }

    xml.push_str("</network>");
    xml
}
