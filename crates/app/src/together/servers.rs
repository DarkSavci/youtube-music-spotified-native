//! The relays a listener has saved: a name for each, so friends can agree
//! on "the same server" without reading an address out.

use serde::{Deserialize, Serialize};

use super::protocol::checked_address;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SavedServer {
    pub id: String,
    pub name: String,
    pub url: String,
}

/// The form that adds a server or edits one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServerForm {
    /// The server being edited; none for a new one.
    pub editing: Option<String>,
    pub name: String,
    pub url: String,
    /// What testing the address came to, or that it is being tested.
    pub check: String,
}

/// What the form's test says while it waits for the server.
pub const CHECKING: &str = "Checking…";
pub const CHECKED: &str = "Ready for v2 rooms";

impl ServerForm {
    /// The form as it opens on `server`: its own name and address, or
    /// empty for a new one.
    pub fn on(server: Option<&SavedServer>) -> Self {
        server.map_or_else(Self::default, |server| Self {
            editing: Some(server.id.clone()),
            name: server.name.clone(),
            url: server.url.clone(),
            check: String::new(),
        })
    }
}

/// The host of a checked address, which names a server that was given no
/// name.
fn host(address: &str) -> &str {
    let rest = address.split_once("://").map_or(address, |(_, rest)| rest);
    rest.split('/').next().unwrap_or(rest)
}

/// An id no saved server has yet.
fn fresh_id(servers: &[SavedServer]) -> String {
    let highest = servers
        .iter()
        .filter_map(|server| server.id.strip_prefix("server-")?.parse::<u64>().ok())
        .max();
    format!("server-{}", highest.map_or(1, |highest| highest + 1))
}

/// Saves what the form holds, as a new server or over the one it edits,
/// and returns its id. The address must be one a room could be reached at.
pub fn save(servers: &mut Vec<SavedServer>, form: &ServerForm) -> Result<String, &'static str> {
    let url = checked_address(&form.url)?;
    let name = match form.name.trim() {
        "" => host(&url).to_owned(),
        name => name.to_owned(),
    };
    let id = match &form.editing {
        Some(id) => id.clone(),
        None => fresh_id(servers),
    };
    servers.retain(|server| server.id != id);
    servers.push(SavedServer {
        id: id.clone(),
        name,
        url,
    });
    Ok(id)
}

/// Brings a profile from before servers had names along: the one address
/// it kept becomes a saved server, and the chosen one.
pub fn adopt(servers: &mut Vec<SavedServer>, selected: &mut String, legacy: &mut String) {
    let address = std::mem::take(legacy);
    if address.trim().is_empty() {
        return;
    }
    if let Some(known) = servers.iter().find(|server| server.url == address.trim()) {
        selected.clone_from(&known.id);
        return;
    }
    let form = ServerForm {
        url: address,
        ..ServerForm::default()
    };
    if let Ok(id) = save(servers, &form) {
        *selected = id;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(name: &str, url: &str) -> ServerForm {
        ServerForm {
            name: name.into(),
            url: url.into(),
            ..ServerForm::default()
        }
    }

    #[test]
    fn a_server_saved_without_a_name_is_called_by_its_host() {
        let mut servers = Vec::new();
        let id = save(&mut servers, &form(" ", "wss://listen.example.com/rooms"));
        assert_eq!(id.as_deref(), Ok("server-1"));
        assert_eq!(servers[0].name, "listen.example.com");
        let id = save(&mut servers, &form("Ours", "ws://localhost:8766"));
        assert_eq!(id.as_deref(), Ok("server-2"));
        assert_eq!(servers[1].name, "Ours");
    }

    #[test]
    fn an_address_no_room_could_be_reached_at_is_not_saved() {
        let mut servers = Vec::new();
        assert!(save(&mut servers, &form("Ours", "http://example.com")).is_err());
        assert!(servers.is_empty());
    }

    #[test]
    fn editing_a_server_replaces_it_under_the_same_id() {
        let mut servers = Vec::new();
        let _ = save(&mut servers, &form("Ours", "ws://localhost:8766"));
        let mut edit = ServerForm::on(servers.first());
        edit.name = "Renamed".into();
        assert_eq!(save(&mut servers, &edit).as_deref(), Ok("server-1"));
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "Renamed");
    }

    #[test]
    fn the_one_address_an_older_profile_kept_becomes_a_saved_server() {
        let (mut servers, mut selected) = (Vec::new(), String::new());
        let mut legacy = "ws://localhost:8791".to_owned();
        adopt(&mut servers, &mut selected, &mut legacy);
        assert!(legacy.is_empty());
        assert_eq!(servers[0].name, "localhost:8791");
        assert_eq!(selected, servers[0].id);
        // Brought along once: the same address again adds nothing.
        let mut legacy = "ws://localhost:8791".to_owned();
        adopt(&mut servers, &mut selected, &mut legacy);
        assert_eq!(servers.len(), 1);
    }
}
