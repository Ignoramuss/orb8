use std::collections::HashSet;

/// Format an IPv4 address from a u32 in little-endian byte order to dotted notation.
///
/// eBPF TC probes read IP addresses with the first octet in the LSB position.
/// On little-endian hosts, `u32::from_le_bytes([a,b,c,d])` stores the first
/// octet in the lowest byte, so we extract bytes via shifting accordingly.
pub fn format_ipv4(ip: u32) -> String {
    format!(
        "{}.{}.{}.{}",
        ip & 0xFF,
        (ip >> 8) & 0xFF,
        (ip >> 16) & 0xFF,
        (ip >> 24) & 0xFF
    )
}

/// Format a protocol number to its human-readable name.
pub fn format_protocol(protocol: u8) -> &'static str {
    match protocol {
        1 => "ICMP",
        6 => "TCP",
        17 => "UDP",
        _ => "OTHER",
    }
}

/// Format a direction byte to its human-readable name.
pub fn format_direction(direction: u8) -> &'static str {
    match direction {
        0 => "ingress",
        1 => "egress",
        _ => "unknown",
    }
}

/// Parse an IPv4 dotted-notation string into a u32 in little-endian byte order.
///
/// Returns the IP with the first octet in the LSB position, matching how eBPF
/// TC probes read packet headers on little-endian systems.
pub fn parse_ipv4(ip_str: &str) -> Option<u32> {
    let parts: Vec<u8> = ip_str.split('.').filter_map(|p| p.parse().ok()).collect();
    if parts.len() == 4 {
        Some(u32::from_le_bytes([parts[0], parts[1], parts[2], parts[3]]))
    } else {
        None
    }
}

/// Discover local IP addresses from `/proc/net/fib_trie`.
///
/// Always includes 127.0.0.1. On non-Linux or if fib_trie is unreadable,
/// returns only the loopback address.
pub fn resolve_local_ips() -> HashSet<u32> {
    let mut ips = HashSet::new();
    ips.insert(u32::from_le_bytes([127, 0, 0, 1]));

    if let Ok(content) = std::fs::read_to_string("/proc/net/fib_trie") {
        let lines: Vec<&str> = content.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if line.trim().starts_with("|-- ") || line.trim().starts_with("+-- ") {
                if let Some(next_line) = lines.get(i + 1) {
                    if next_line.contains("/32 host LOCAL") {
                        let ip_str = line
                            .trim()
                            .trim_start_matches("|-- ")
                            .trim_start_matches("+-- ");
                        if let Some(ip) = parse_ipv4(ip_str) {
                            ips.insert(ip);
                        }
                    }
                }
            }
        }
    }
    ips
}

/// Check if a network event is self-traffic (agent's own gRPC connections).
///
/// When local IPs are available, matches on both port AND IP to avoid
/// filtering out legitimate application traffic on the same port.
/// Falls back to port-only matching when local IPs couldn't be resolved.
pub fn is_self_traffic(
    event: &orb8_common::NetworkFlowEvent,
    grpc_port: u16,
    local_ips: &HashSet<u32>,
) -> bool {
    if local_ips.is_empty() {
        return event.src_port == grpc_port || event.dst_port == grpc_port;
    }
    (event.src_port == grpc_port && local_ips.contains(&event.src_ip))
        || (event.dst_port == grpc_port && local_ips.contains(&event.dst_ip))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_ipv4() {
        // 10.0.0.5 stored as LE: [10, 0, 0, 5] -> 0x0500000A
        assert_eq!(format_ipv4(0x0500000A), "10.0.0.5");
        // 192.168.1.100 stored as LE: [192, 168, 1, 100] -> 0x6401A8C0
        assert_eq!(format_ipv4(0x6401A8C0), "192.168.1.100");
        // 127.0.0.1
        assert_eq!(format_ipv4(0x0100007F), "127.0.0.1");
    }

    #[test]
    fn test_format_protocol() {
        assert_eq!(format_protocol(6), "TCP");
        assert_eq!(format_protocol(17), "UDP");
        assert_eq!(format_protocol(1), "ICMP");
        assert_eq!(format_protocol(99), "OTHER");
    }

    #[test]
    fn test_format_direction() {
        assert_eq!(format_direction(0), "ingress");
        assert_eq!(format_direction(1), "egress");
        assert_eq!(format_direction(2), "unknown");
    }

    #[test]
    fn test_parse_ipv4() {
        assert_eq!(parse_ipv4("10.0.0.5"), Some(0x0500000A));
        assert_eq!(parse_ipv4("192.168.1.100"), Some(0x6401A8C0));
        assert_eq!(parse_ipv4("172.18.0.2"), Some(0x020012AC));
        assert_eq!(parse_ipv4("127.0.0.1"), Some(0x0100007F));
    }

    #[test]
    fn test_parse_ipv4_invalid() {
        assert_eq!(parse_ipv4(""), None);
        assert_eq!(parse_ipv4("10.0.0"), None);
        assert_eq!(parse_ipv4("not.an.ip.addr"), None);
        assert_eq!(parse_ipv4("256.0.0.1"), None);
    }

    #[test]
    fn test_parse_format_roundtrip() {
        for ip_str in &["10.0.0.5", "192.168.1.100", "172.18.0.2", "127.0.0.1"] {
            let parsed = parse_ipv4(ip_str).unwrap();
            assert_eq!(format_ipv4(parsed), *ip_str);
        }
    }

    #[test]
    fn test_is_self_traffic() {
        let mut local_ips = HashSet::new();
        local_ips.insert(u32::from_le_bytes([10, 0, 0, 1]));

        let local_ip = u32::from_le_bytes([10, 0, 0, 1]);
        let remote_ip = u32::from_le_bytes([10, 0, 0, 99]);

        let event = orb8_common::NetworkFlowEvent {
            src_ip: local_ip,
            dst_ip: remote_ip,
            src_port: 9090,
            dst_port: 12345,
            protocol: 6,
            direction: 1,
            packet_len: 100,
            cgroup_id: 0,
            timestamp_ns: 0,
        };
        assert!(is_self_traffic(&event, 9090, &local_ips));

        let event2 = orb8_common::NetworkFlowEvent {
            src_ip: remote_ip,
            dst_ip: local_ip,
            src_port: 12345,
            dst_port: 9090,
            protocol: 6,
            direction: 0,
            packet_len: 100,
            cgroup_id: 0,
            timestamp_ns: 0,
        };
        assert!(is_self_traffic(&event2, 9090, &local_ips));

        // Remote IP on port 9090 should NOT be filtered
        let event3 = orb8_common::NetworkFlowEvent {
            src_ip: remote_ip,
            dst_ip: local_ip,
            src_port: 9090,
            dst_port: 8080,
            protocol: 6,
            direction: 0,
            packet_len: 100,
            cgroup_id: 0,
            timestamp_ns: 0,
        };
        assert!(!is_self_traffic(&event3, 9090, &local_ips));
    }

    #[test]
    fn test_is_self_traffic_empty_local_ips() {
        let empty: HashSet<u32> = HashSet::new();
        let event = orb8_common::NetworkFlowEvent {
            src_ip: 0,
            dst_ip: 0,
            src_port: 9090,
            dst_port: 12345,
            protocol: 6,
            direction: 1,
            packet_len: 100,
            cgroup_id: 0,
            timestamp_ns: 0,
        };
        // Falls back to port-only matching
        assert!(is_self_traffic(&event, 9090, &empty));
    }
}
