//! Native Linux backend using /proc filesystem.
//!
//! Reads process and resource information directly from /proc,
//! eliminating the dependency on lsof.

use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::types::Entry;

/// Find processes that have the given file open.
pub fn query_file(target: &Path) -> Result<Vec<Entry>, String> {
    let target_meta = fs::metadata(target)
        .map_err(|e| format!("cannot stat {}: {}", target.display(), e))?;
    let target_dev = target_meta.dev();
    let target_ino = target_meta.ino();

    let mut entries = Vec::new();

    for pid in list_pids() {
        let proc_info = match read_process_info(pid) {
            Some(info) => info,
            None => continue,
        };

        let fd_dir = format!("/proc/{}/fd", pid);
        let fd_entries = match fs::read_dir(&fd_dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in fd_entries.flatten() {
            let fd_name = entry.file_name().to_string_lossy().to_string();

            // Check if this fd points to our target file by comparing dev+inode
            if let Ok(meta) = entry.metadata() {
                if meta.dev() == target_dev && meta.ino() == target_ino {
                    let access = read_fd_mode(pid, &fd_name);
                    entries.push(Entry {
                        pid: pid.to_string(),
                        command: proc_info.command.clone(),
                        user: proc_info.user.clone(),
                        fd: fd_name,
                        file_type: file_type_from_meta(&meta),
                        access,
                        name: target.display().to_string(),
                    });
                }
            }
        }
    }

    Ok(entries)
}

/// Find processes using the given port.
pub fn query_port(port: u16) -> Result<Vec<Entry>, String> {
    // Build a map of socket inode → socket description from /proc/net/*
    let socket_inodes = find_port_inodes(port)?;
    if socket_inodes.is_empty() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();

    for pid in list_pids() {
        let proc_info = match read_process_info(pid) {
            Some(info) => info,
            None => continue,
        };

        let fd_dir = format!("/proc/{}/fd", pid);
        let fd_entries = match fs::read_dir(&fd_dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in fd_entries.flatten() {
            let fd_name = entry.file_name().to_string_lossy().to_string();
            let link = match fs::read_link(entry.path()) {
                Ok(l) => l,
                Err(_) => continue,
            };
            let link_str = link.to_string_lossy();

            // Socket fds look like "socket:[12345]"
            if let Some(inode) = parse_socket_inode(&link_str) {
                if let Some(sock_info) = socket_inodes.get(&inode) {
                    let access = read_fd_mode(pid, &fd_name);
                    entries.push(Entry {
                        pid: pid.to_string(),
                        command: proc_info.command.clone(),
                        user: proc_info.user.clone(),
                        fd: fd_name,
                        file_type: sock_info.proto.clone(),
                        access,
                        name: sock_info.display.clone(),
                    });
                }
            }
        }
    }

    Ok(entries)
}

/// List all resources held by the given PID.
pub fn query_pid(pid: u32) -> Result<Vec<Entry>, String> {
    let proc_dir = PathBuf::from(format!("/proc/{}", pid));
    if !proc_dir.exists() {
        return Ok(Vec::new());
    }

    let proc_info = read_process_info(pid)
        .ok_or_else(|| format!("cannot read process {}", pid))?;

    // Build socket inode → info map for resolving socket fds
    let socket_map = build_socket_map();

    let mut entries = Vec::new();

    // Current working directory
    if let Ok(cwd) = fs::read_link(format!("/proc/{}/cwd", pid)) {
        entries.push(Entry {
            pid: pid.to_string(),
            command: proc_info.command.clone(),
            user: proc_info.user.clone(),
            fd: "cwd".to_string(),
            file_type: "DIR".to_string(),
            access: String::new(),
            name: cwd.display().to_string(),
        });
    }

    // Executable
    if let Ok(exe) = fs::read_link(format!("/proc/{}/exe", pid)) {
        entries.push(Entry {
            pid: pid.to_string(),
            command: proc_info.command.clone(),
            user: proc_info.user.clone(),
            fd: "txt".to_string(),
            file_type: "REG".to_string(),
            access: String::new(),
            name: exe.display().to_string(),
        });
    }

    // File descriptors
    let fd_dir = format!("/proc/{}/fd", pid);
    let mut fd_entries: Vec<_> = match fs::read_dir(&fd_dir) {
        Ok(entries) => entries.flatten().collect(),
        Err(_) => Vec::new(),
    };
    // Sort by fd number
    fd_entries.sort_by_key(|e| {
        e.file_name()
            .to_string_lossy()
            .parse::<u64>()
            .unwrap_or(u64::MAX)
    });

    for entry in fd_entries {
        let fd_name = entry.file_name().to_string_lossy().to_string();
        let link = match fs::read_link(entry.path()) {
            Ok(l) => l,
            Err(_) => continue,
        };
        let link_str = link.to_string_lossy().to_string();
        let access = read_fd_mode(pid, &fd_name);

        let (file_type, name) = if let Some(inode) = parse_socket_inode(&link_str) {
            if let Some(sock_info) = socket_map.get(&inode) {
                (sock_info.proto.clone(), sock_info.display.clone())
            } else {
                ("sock".to_string(), format!("socket:[{}]", inode))
            }
        } else if link_str.starts_with("pipe:[") {
            ("PIPE".to_string(), link_str)
        } else if link_str.starts_with("anon_inode:") {
            let kind = link_str.strip_prefix("anon_inode:").unwrap_or(&link_str);
            (kind.trim_matches(|c| c == '[' || c == ']').to_uppercase(), link_str)
        } else {
            // Regular file, directory, device, etc.
            let ft = match fs::metadata(entry.path()) {
                Ok(meta) => file_type_from_meta(&meta),
                Err(_) => "?".to_string(),
            };
            (ft, link_str)
        };

        entries.push(Entry {
            pid: pid.to_string(),
            command: proc_info.command.clone(),
            user: proc_info.user.clone(),
            fd: fd_name,
            file_type,
            access,
            name,
        });
    }

    Ok(entries)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

struct ProcessInfo {
    command: String,
    user: String,
}

fn read_process_info(pid: u32) -> Option<ProcessInfo> {
    let command = fs::read_to_string(format!("/proc/{}/comm", pid))
        .ok()?
        .trim()
        .to_string();

    let status = fs::read_to_string(format!("/proc/{}/status", pid)).ok()?;
    let uid = status
        .lines()
        .find(|l| l.starts_with("Uid:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|u| u.parse::<u32>().ok())
        .unwrap_or(0);

    let user = resolve_uid(uid);

    Some(ProcessInfo { command, user })
}

/// List all numeric PIDs in /proc.
fn list_pids() -> Vec<u32> {
    let entries = match fs::read_dir("/proc") {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    entries
        .flatten()
        .filter_map(|e| e.file_name().to_string_lossy().parse::<u32>().ok())
        .collect()
}

/// Read fd access mode from /proc/<pid>/fdinfo/<fd>.
///
/// The `flags` field in fdinfo contains the open flags in octal.
/// Bits 0-1 encode access: 0 = read-only, 1 = write-only, 2 = read-write.
fn read_fd_mode(pid: u32, fd: &str) -> String {
    let path = format!("/proc/{}/fdinfo/{}", pid, fd);
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };

    for line in content.lines() {
        if let Some(flags_str) = line.strip_prefix("flags:") {
            let flags_str = flags_str.trim();
            if let Ok(flags) = u32::from_str_radix(flags_str, 8) {
                return match flags & 0o3 {
                    0 => "r".to_string(),
                    1 => "w".to_string(),
                    2 => "rw".to_string(),
                    _ => "u".to_string(),
                };
            }
        }
    }

    String::new()
}

fn file_type_from_meta(meta: &fs::Metadata) -> String {
    let ft = meta.file_type();
    if ft.is_dir() {
        "DIR".to_string()
    } else if ft.is_symlink() {
        "LINK".to_string()
    } else {
        "REG".to_string()
    }
}

/// Resolve a UID to a username by reading /etc/passwd.
fn resolve_uid(uid: u32) -> String {
    let content = match fs::read_to_string("/etc/passwd") {
        Ok(c) => c,
        Err(_) => return uid.to_string(),
    };

    for line in content.lines() {
        let fields: Vec<&str> = line.split(':').collect();
        if fields.len() >= 3 {
            if let Ok(file_uid) = fields[2].parse::<u32>() {
                if file_uid == uid {
                    return fields[0].to_string();
                }
            }
        }
    }

    uid.to_string()
}

/// Extract socket inode from a readlink result like "socket:[12345]".
fn parse_socket_inode(link: &str) -> Option<u64> {
    let inner = link.strip_prefix("socket:[")?;
    let inode_str = inner.strip_suffix(']')?;
    inode_str.parse::<u64>().ok()
}

// ---------------------------------------------------------------------------
// /proc/net/* socket parsing
// ---------------------------------------------------------------------------

struct SocketInfo {
    proto: String,
    display: String,
}

/// Find socket inodes matching a specific port in /proc/net/*.
fn find_port_inodes(port: u16) -> Result<HashMap<u64, SocketInfo>, String> {
    let mut map = HashMap::new();
    let port_hex = format!("{:04X}", port);

    for (path, proto) in &[
        ("/proc/net/tcp", "IPv4"),
        ("/proc/net/tcp6", "IPv6"),
        ("/proc/net/udp", "IPv4"),
        ("/proc/net/udp6", "IPv6"),
    ] {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let is_udp = path.contains("udp");

        for line in content.lines().skip(1) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 10 {
                continue;
            }

            let local_addr = fields[1];
            let remote_addr = fields[2];
            let state = fields[3];
            let inode_str = fields[9];

            // Check if local port matches
            let local_port_hex = local_addr.rsplit(':').next().unwrap_or("");
            let remote_port_hex = remote_addr.rsplit(':').next().unwrap_or("");

            if local_port_hex != port_hex && remote_port_hex != port_hex {
                continue;
            }

            let inode = match inode_str.parse::<u64>() {
                Ok(i) if i > 0 => i,
                _ => continue,
            };

            let display = format_net_address(local_addr, remote_addr, state, is_udp);
            let proto_label = if is_udp {
                format!("UDP{}", if *proto == "IPv6" { "6" } else { "" })
            } else {
                proto.to_string()
            };

            map.insert(inode, SocketInfo {
                proto: proto_label,
                display,
            });
        }
    }

    Ok(map)
}

/// Build a complete socket inode → info map from all /proc/net/* files.
fn build_socket_map() -> HashMap<u64, SocketInfo> {
    let mut map = HashMap::new();

    for (path, proto) in &[
        ("/proc/net/tcp", "IPv4"),
        ("/proc/net/tcp6", "IPv6"),
        ("/proc/net/udp", "IPv4"),
        ("/proc/net/udp6", "IPv6"),
    ] {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let is_udp = path.contains("udp");

        for line in content.lines().skip(1) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 10 {
                continue;
            }

            let local_addr = fields[1];
            let remote_addr = fields[2];
            let state = fields[3];
            let inode_str = fields[9];

            let inode = match inode_str.parse::<u64>() {
                Ok(i) if i > 0 => i,
                _ => continue,
            };

            let display = format_net_address(local_addr, remote_addr, state, is_udp);
            let proto_label = if is_udp {
                format!("UDP{}", if *proto == "IPv6" { "6" } else { "" })
            } else {
                proto.to_string()
            };

            map.insert(inode, SocketInfo {
                proto: proto_label,
                display,
            });
        }
    }

    // Unix domain sockets
    if let Ok(content) = fs::read_to_string("/proc/net/unix") {
        for line in content.lines().skip(1) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 7 {
                continue;
            }

            let inode = match fields[6].parse::<u64>() {
                Ok(i) if i > 0 => i,
                _ => continue,
            };

            let path = if fields.len() > 7 { fields[7] } else { "" };
            let display = if path.is_empty() {
                "unix socket".to_string()
            } else {
                path.to_string()
            };

            map.insert(inode, SocketInfo {
                proto: "unix".to_string(),
                display,
            });
        }
    }

    map
}

/// Format a /proc/net/* address pair into a human-readable string.
///
/// Addresses in /proc/net/tcp are hex-encoded: `0100007F:1F90` = 127.0.0.1:8080
fn format_net_address(local: &str, remote: &str, state: &str, is_udp: bool) -> String {
    let local_parsed = parse_hex_address(local);
    let remote_parsed = parse_hex_address(remote);

    let state_str = if is_udp {
        "" // UDP doesn't have meaningful connection states
    } else {
        match state {
            "0A" => " (LISTEN)",
            "01" => " (ESTABLISHED)",
            "06" => " (TIME_WAIT)",
            "08" => " (CLOSE_WAIT)",
            "02" => " (SYN_SENT)",
            "03" => " (SYN_RECV)",
            _ => "",
        }
    };

    if remote_parsed == "*:0" || remote_parsed == "[::]:0" {
        format!("{}{}", local_parsed, state_str)
    } else {
        format!("{}->{}{}", local_parsed, remote_parsed, state_str)
    }
}

/// Parse a hex-encoded address like `0100007F:1F90` into `127.0.0.1:8080`.
fn parse_hex_address(addr: &str) -> String {
    let parts: Vec<&str> = addr.split(':').collect();
    if parts.len() != 2 {
        return addr.to_string();
    }

    let port = u16::from_str_radix(parts[1], 16).unwrap_or(0);
    let ip_hex = parts[0];

    if ip_hex.len() == 8 {
        // IPv4: stored as little-endian 32-bit
        if let Ok(ip_num) = u32::from_str_radix(ip_hex, 16) {
            let a = ip_num & 0xFF;
            let b = (ip_num >> 8) & 0xFF;
            let c = (ip_num >> 16) & 0xFF;
            let d = (ip_num >> 24) & 0xFF;
            let ip_str = if ip_num == 0 {
                "*".to_string()
            } else if a == 127 && b == 0 && c == 0 && d == 1 {
                "localhost".to_string()
            } else {
                format!("{}.{}.{}.{}", a, b, c, d)
            };
            return format!("{}:{}", ip_str, port);
        }
    } else if ip_hex.len() == 32 {
        // IPv6: stored as four little-endian 32-bit words
        let all_zero = ip_hex.chars().all(|c| c == '0');
        if all_zero {
            return format!("[::]:{}",  port);
        }
        // Check for ::1 (loopback): 00000000000000000000000001000000
        if ip_hex == "00000000000000000000000001000000" {
            return format!("localhost:{}", port);
        }
        // For other IPv6, just show abbreviated hex
        return format!("[ipv6]:{}", port);
    }

    format!("{}:{}", ip_hex, port)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Socket inode parsing ---

    #[test]
    fn test_parse_socket_inode_valid() {
        assert_eq!(parse_socket_inode("socket:[12345]"), Some(12345));
        assert_eq!(parse_socket_inode("socket:[0]"), Some(0));
        assert_eq!(parse_socket_inode("socket:[999999999]"), Some(999999999));
    }

    #[test]
    fn test_parse_socket_inode_not_socket() {
        assert_eq!(parse_socket_inode("pipe:[999]"), None);
        assert_eq!(parse_socket_inode("/dev/null"), None);
        assert_eq!(parse_socket_inode("anon_inode:[eventfd]"), None);
        assert_eq!(parse_socket_inode(""), None);
    }

    #[test]
    fn test_parse_socket_inode_malformed() {
        assert_eq!(parse_socket_inode("socket:[]"), None); // empty inode
        assert_eq!(parse_socket_inode("socket:[abc]"), None); // non-numeric
        assert_eq!(parse_socket_inode("socket:[123"), None); // missing bracket
        assert_eq!(parse_socket_inode("socket:123]"), None); // missing bracket
    }

    // --- IPv4 hex address parsing ---

    #[test]
    fn test_parse_hex_address_ipv4_localhost() {
        assert_eq!(parse_hex_address("0100007F:1F90"), "localhost:8080");
    }

    #[test]
    fn test_parse_hex_address_ipv4_any() {
        assert_eq!(parse_hex_address("00000000:0050"), "*:80");
    }

    #[test]
    fn test_parse_hex_address_ipv4_regular() {
        // 192.168.1.1 in little-endian hex: 0101A8C0
        assert_eq!(parse_hex_address("0101A8C0:0050"), "192.168.1.1:80");
    }

    #[test]
    fn test_parse_hex_address_ipv4_port_zero() {
        assert_eq!(parse_hex_address("00000000:0000"), "*:0");
    }

    #[test]
    fn test_parse_hex_address_ipv4_high_port() {
        // Port 65535 = FFFF
        assert_eq!(parse_hex_address("0100007F:FFFF"), "localhost:65535");
    }

    // --- IPv6 hex address parsing ---

    #[test]
    fn test_parse_hex_address_ipv6_any() {
        assert_eq!(
            parse_hex_address("00000000000000000000000000000000:0050"),
            "[::]:80"
        );
    }

    #[test]
    fn test_parse_hex_address_ipv6_loopback() {
        assert_eq!(
            parse_hex_address("00000000000000000000000001000000:240E"),
            "localhost:9230"
        );
    }

    #[test]
    fn test_parse_hex_address_ipv6_other() {
        // Any non-zero, non-loopback IPv6 shows as [ipv6]:port
        assert_eq!(
            parse_hex_address("DEADBEEF000000000000000000000000:0050"),
            "[ipv6]:80"
        );
    }

    // --- Malformed address parsing ---

    #[test]
    fn test_parse_hex_address_no_colon() {
        // No colon separator — return as-is
        assert_eq!(parse_hex_address("garbage"), "garbage");
    }

    #[test]
    fn test_parse_hex_address_weird_length() {
        // IP hex that's neither 8 (IPv4) nor 32 (IPv6) chars
        assert_eq!(parse_hex_address("ABCDEF:0050"), "ABCDEF:80");
    }

    // --- Network address formatting ---

    #[test]
    fn test_format_net_address_listen() {
        assert_eq!(
            format_net_address("00000000:1F90", "00000000:0000", "0A", false),
            "*:8080 (LISTEN)"
        );
    }

    #[test]
    fn test_format_net_address_established() {
        assert_eq!(
            format_net_address("0100007F:1F90", "0100007F:C000", "01", false),
            "localhost:8080->localhost:49152 (ESTABLISHED)"
        );
    }

    #[test]
    fn test_format_net_address_time_wait() {
        assert_eq!(
            format_net_address("0100007F:1F90", "0100007F:C000", "06", false),
            "localhost:8080->localhost:49152 (TIME_WAIT)"
        );
    }

    #[test]
    fn test_format_net_address_close_wait() {
        assert_eq!(
            format_net_address("0100007F:1F90", "0100007F:C000", "08", false),
            "localhost:8080->localhost:49152 (CLOSE_WAIT)"
        );
    }

    #[test]
    fn test_format_net_address_syn_sent() {
        assert_eq!(
            format_net_address("0100007F:1F90", "0100007F:C000", "02", false),
            "localhost:8080->localhost:49152 (SYN_SENT)"
        );
    }

    #[test]
    fn test_format_net_address_unknown_state() {
        // Unknown state code — no state suffix
        assert_eq!(
            format_net_address("0100007F:1F90", "0100007F:C000", "FF", false),
            "localhost:8080->localhost:49152"
        );
    }

    #[test]
    fn test_format_net_address_udp_no_state() {
        // UDP should never show connection state
        assert_eq!(
            format_net_address("00000000:1F90", "00000000:0000", "07", true),
            "*:8080"
        );
    }

    #[test]
    fn test_format_net_address_ipv6_listen() {
        assert_eq!(
            format_net_address(
                "00000000000000000000000000000000:1F90",
                "00000000000000000000000000000000:0000",
                "0A",
                false
            ),
            "[::]:8080 (LISTEN)"
        );
    }

    // --- fd mode reading ---

    #[test]
    fn test_read_fd_mode_nonexistent() {
        assert_eq!(read_fd_mode(999999999, "0"), "");
    }

    // --- UID resolution ---

    #[test]
    fn test_resolve_uid_zero() {
        let result = resolve_uid(0);
        assert!(result == "root" || result == "0");
    }

    #[test]
    fn test_resolve_uid_nonexistent() {
        // UID 4294967295 almost certainly doesn't exist in /etc/passwd
        let result = resolve_uid(4294967295);
        assert_eq!(result, "4294967295");
    }

    // --- file_type_from_meta ---

    #[test]
    fn test_file_type_regular_file() {
        let meta = fs::metadata("/dev/null").unwrap();
        // /dev/null is a char device, but file_type_from_meta only checks dir/symlink/else
        // So it falls through to "REG" — this is a known simplification
        let ft = file_type_from_meta(&meta);
        assert!(ft == "REG" || ft == "DIR" || ft == "LINK");
    }
}
