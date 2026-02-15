use virt::domain_snapshot::DomainSnapshot;
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::backend::domain::with_domain;
use crate::backend::types::{CreateSnapshotParams, SnapshotInfo, SnapshotState};
use crate::error::AppError;

pub fn list_snapshots(uri: &str, uuid: &str) -> Result<Vec<SnapshotInfo>, AppError> {
    with_domain(uri, uuid, |domain| {
        let snapshots = domain.list_all_snapshots(0)?;

        // Get current snapshot name, if any
        let current_name = DomainSnapshot::current(domain, 0)
            .ok()
            .and_then(|snap| snap.get_name().ok());

        let mut infos: Vec<SnapshotInfo> = Vec::new();
        for snap in &snapshots {
            let xml = snap.get_xml_desc(0)?;
            if let Some(mut info) = parse_snapshot_xml(&xml) {
                info.is_current = current_name.as_deref() == Some(&info.name);
                infos.push(info);
            }
        }

        // Sort newest first
        infos.sort_by(|a, b| b.creation_time.cmp(&a.creation_time));
        Ok(infos)
    })
}

pub fn create_snapshot(
    uri: &str,
    uuid: &str,
    params: &CreateSnapshotParams,
) -> Result<(), AppError> {
    with_domain(uri, uuid, |domain| {
        let xml = format!(
            "<domainsnapshot>\n  <name>{}</name>\n  <description>{}</description>\n</domainsnapshot>",
            escape_xml(&params.name),
            escape_xml(&params.description),
        );
        DomainSnapshot::create_xml(domain, &xml, 0)?;
        Ok(())
    })
}

pub fn delete_snapshot(uri: &str, uuid: &str, snap_name: &str) -> Result<(), AppError> {
    with_domain(uri, uuid, |domain| {
        let snap = DomainSnapshot::lookup_by_name(domain, snap_name, 0)?;
        snap.delete(0)?;
        Ok(())
    })
}

pub fn revert_snapshot(uri: &str, uuid: &str, snap_name: &str) -> Result<(), AppError> {
    with_domain(uri, uuid, |domain| {
        let snap = DomainSnapshot::lookup_by_name(domain, snap_name, 0)?;
        snap.revert(0)?;
        Ok(())
    })
}

fn parse_snapshot_xml(xml: &str) -> Option<SnapshotInfo> {
    let mut reader = Reader::from_str(xml);

    let mut name = String::new();
    let mut description = String::new();
    let mut state = SnapshotState::Other;
    let mut creation_time: i64 = 0;

    enum Context {
        None,
        Name,
        Description,
        State,
        CreationTime,
    }

    let mut ctx = Context::None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => match e.name().as_ref() {
                b"name" => ctx = Context::Name,
                b"description" => ctx = Context::Description,
                b"state" => ctx = Context::State,
                b"creationTime" => ctx = Context::CreationTime,
                _ => {}
            },
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                match ctx {
                    Context::Name => name = text,
                    Context::Description => description = text,
                    Context::State => state = SnapshotState::from_xml_str(&text),
                    Context::CreationTime => {
                        creation_time = text.trim().parse().unwrap_or(0);
                    }
                    Context::None => {}
                }
            }
            Ok(Event::End(_)) => {
                ctx = Context::None;
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    if name.is_empty() {
        return None;
    }

    Some(SnapshotInfo {
        name,
        description,
        state,
        creation_time,
        is_current: false,
    })
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
