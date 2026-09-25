use boson::{dht::Node, Id, NodeInfo};
use clap::{Arg, ArgMatches, Command};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CachedEntry {
    #[serde(rename = "nodeinfo")]
    pub node_info: NodeInfo,
    #[serde(default)]
    pub created: u64,
    #[serde(rename = "lastSeen", default)]
    pub last_seen: u64,
    #[serde(rename = "lastSent", default)]
    pub last_sent: u64,
    #[serde(default)]
    pub reachable: bool,
    #[serde(rename = "version", default)]
    pub version: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CachedRoutingTable {
    #[serde(rename = "nodeId")]
    pub node_id: Id,
    #[serde(default)]
    pub timestamp: u64,
    #[serde(default)]
    pub entries: Vec<CachedEntry>,
}

pub(crate) fn command() -> Command {
    Command::new("rt")
        .about("Display nodes in the routing table for dht4 and dht6 from cache files")
        .arg(
            Arg::new("ipv4")
                .short('4')
                .long("ipv4")
                .help("Display only IPv4 (dht4) routing table")
                .action(clap::ArgAction::SetTrue)
                .conflicts_with("ipv6"),
        )
        .arg(
            Arg::new("ipv6")
                .short('6')
                .long("ipv6")
                .help("Display only IPv6 (dht6) routing table")
                .action(clap::ArgAction::SetTrue)
                .conflicts_with("ipv4"),
        )
        .arg(
            Arg::new("file")
                .short('f')
                .long("file")
                .value_name("PATH")
                .help("Read routing table from a specific cache file"),
        )
}

fn expand_path(p: &str) -> PathBuf {
    if p == "~" {
        dirs_home().unwrap_or_else(|| PathBuf::from(p))
    } else if let Some(stripped) = p.strip_prefix("~/") {
        match dirs_home() {
            Some(home) => home.join(stripped),
            _ => PathBuf::from(p),
        }
    } else {
        PathBuf::from(p)
    }
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn format_duration_ms(ms: u64, now_ms: u64) -> String {
    if ms == 0 {
        return "never".to_string();
    }
    if ms > now_ms {
        return "in the future".to_string();
    }
    let elapsed = (now_ms - ms) / 1000;
    if elapsed < 60 {
        format!("{elapsed}s ago")
    } else if elapsed < 3600 {
        format!("{}m {}s ago", elapsed / 60, elapsed % 60)
    } else if elapsed < 86400 {
        format!("{}h {}m ago", elapsed / 3600, (elapsed % 3600) / 60)
    } else {
        format!("{}d ago", elapsed / 86400)
    }
}

fn format_version(ver: i32) -> String {
    let u = ver as u32;
    if u == 0 {
        return "N/A".to_string();
    }
    let b0 = (u >> 24) as u8;
    let b1 = ((u >> 16) & 0xFF) as u8;
    let num = u & 0xFFFF;
    if b0.is_ascii_alphanumeric() && b1.is_ascii_alphanumeric() {
        format!("{}{}/{}", b0 as char, b1 as char, num)
    } else {
        format!("{ver}")
    }
}

pub(crate) fn display_cache_file(path: &Path, title: &str) {
    println!("+------------------------------------------------------------+");
    println!("| {:<58} |", title);
    println!("+------------------------------------------------------------+");
    println!("Cache File:  {}", path.display());

    if !path.exists() {
        println!("Status:      \x1b[33mFile not found (no routing table cached yet)\x1b[0m");
        return;
    }

    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            println!("Status:      \x1b[31mFailed to read file: {e}\x1b[0m");
            return;
        }
    };

    if bytes.is_empty() {
        println!("Status:      \x1b[33mEmpty cache file (0 entries)\x1b[0m");
        return;
    }

    let rt: CachedRoutingTable = match serde_cbor::from_slice(&bytes) {
        Ok(rt) => rt,
        Err(e) => {
            println!("Status:      \x1b[31mFailed to parse CBOR routing table: {e}\x1b[0m");
            return;
        }
    };

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    println!("Node ID:     {}", rt.node_id);
    println!(
        "Cached:      {}",
        if rt.timestamp > 0 {
            format_duration_ms(rt.timestamp, now_ms)
        } else {
            "unknown".to_string()
        }
    );
    println!("Total Nodes: {}", rt.entries.len());

    if rt.entries.is_empty() {
        println!("Nodes:       (no nodes in routing table)");
        return;
    }

    println!();
    for (idx, entry) in rt.entries.iter().enumerate() {
        let reachable_str = if entry.reachable {
            "\x1b[32myes\x1b[0m"
        } else {
            "\x1b[33mno\x1b[0m"
        };
        let ver_str = format_version(entry.version);
        let seen_str = format_duration_ms(entry.last_seen, now_ms);
        let sent_str = format_duration_ms(entry.last_sent, now_ms);
        let age_str = format_duration_ms(entry.created, now_ms);

        println!(
            "[{}] {} @ {}",
            idx + 1,
            entry.node_info.id(),
            entry.node_info.address()
        );
        println!(
            "    Reachable: {} | Version: {} | Last Seen: {} | Last Sent: {} | Age: {}",
            reachable_str, ver_str, seen_str, sent_str, age_str
        );
    }
}

pub(crate) fn run(matches: &ArgMatches, node: &Node) {
    if let Some(custom_file) = matches.get_one::<String>("file") {
        let path = expand_path(custom_file);
        display_cache_file(&path, "Custom Routing Table");
        return;
    }

    let show_ipv4_only = matches.get_flag("ipv4");
    let show_ipv6_only = matches.get_flag("ipv6");

    let show_ipv4 = show_ipv4_only || !show_ipv6_only;
    let show_ipv6 = show_ipv6_only || !show_ipv4_only;

    let data_dir = expand_path(node.options().data_dir());
    let dht4_cache = data_dir.join("dht4.cache");
    let dht6_cache = data_dir.join("dht6.cache");

    if show_ipv4 {
        display_cache_file(&dht4_cache, "DHT4 (IPv4) Routing Table");
    }

    if show_ipv4 && show_ipv6 {
        println!();
    }

    if show_ipv6 {
        display_cache_file(&dht6_cache, "DHT6 (IPv6) Routing Table");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_version() {
        assert_eq!(format_version(0), "N/A");
        // 'M' = 0x4D, 'K' = 0x4B, 1 = 0x0001
        // (0x4D << 24) | (0x4B << 16) | 1 = 0x4D4B0001
        let mk1 = ((b'M' as u32) << 24) | ((b'K' as u32) << 16) | 1;
        assert_eq!(format_version(mk1 as i32), "MK/1");
    }

    #[test]
    fn test_format_duration_ms() {
        assert_eq!(format_duration_ms(0, 1000), "never");
        assert_eq!(format_duration_ms(1000, 5000), "4s ago");
        assert_eq!(format_duration_ms(1000, 65000), "1m 4s ago");
    }

    #[test]
    fn test_deserialize_cached_routing_table() {
        let node_id = Id::random();
        let addr = "127.0.0.1:39001".parse().unwrap();
        let ni = NodeInfo::new(node_id.clone(), addr);

        let entry = CachedEntry {
            node_info: ni,
            created: 1000,
            last_seen: 2000,
            last_sent: 1500,
            reachable: true,
            version: 0,
        };

        let rt = CachedRoutingTable {
            node_id: node_id.clone(),
            timestamp: 5000,
            entries: vec![entry],
        };

        let bytes = serde_cbor::to_vec(&rt).unwrap();
        let deserialized: CachedRoutingTable = serde_cbor::from_slice(&bytes).unwrap();
        assert_eq!(deserialized.node_id, node_id);
        assert_eq!(deserialized.timestamp, 5000);
        assert_eq!(deserialized.entries.len(), 1);
        assert_eq!(deserialized.entries[0].node_info.id(), &node_id);
        assert_eq!(deserialized.entries[0].reachable, true);
    }

    #[test]
    fn test_read_real_cache_file_if_present() {
        if let Some(home) = dirs_home() {
            let path = home.join(".boson_shell").join("dht4.cache");
            if path.exists() {
                display_cache_file(&path, "DHT4 (IPv4) Routing Table");
            }
        }
    }
}
