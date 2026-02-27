#[derive(Debug, Clone)]
pub struct OsEntry {
    pub short_id: String,
    pub name: String,
    pub id: String, // full libosinfo URI, e.g. "http://fedoraproject.org/fedora/40"
}

/// Look up the human-readable OS name for a given libosinfo URI.
/// Returns None if osinfo-query is unavailable or the URI is not found.
pub fn lookup_os_name_by_uri(uri: &str) -> Option<String> {
    load_os_list().into_iter().find(|e| e.id == uri).map(|e| e.name)
}

/// Load all OS entries from osinfo-db via osinfo-query.
/// Returns an empty Vec if osinfo-query is unavailable or fails.
pub fn load_os_list() -> Vec<OsEntry> {
    let output = match std::process::Command::new("osinfo-query")
        .args(["os", "--fields=short-id,name,id"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    if !output.status.success() {
        return Vec::new();
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();

    // Output format (first 2 lines are header + separator, skip them):
    //  Short ID             | Name                         | ID
    // ----------------------+------------------------------+-----
    //  fedora40             | Fedora 40                    | http://...
    for line in text.lines().skip(2) {
        let parts: Vec<&str> = line.splitn(3, '|').collect();
        if parts.len() < 3 {
            continue;
        }
        let short_id = parts[0].trim().to_string();
        let name = parts[1].trim().to_string();
        let id = parts[2].trim().to_string();

        if short_id.is_empty() || name.is_empty() || short_id.starts_with('-') {
            continue;
        }

        entries.push(OsEntry { short_id, name, id });
    }

    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    entries
}
