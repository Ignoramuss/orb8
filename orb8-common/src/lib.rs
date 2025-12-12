//! Shared types between eBPF (kernel) and userspace
//!
//! This crate defines event structures that must be:
//! - `#[repr(C)]` for stable memory layout
//! - `no_std` compatible for eBPF
//! - Shared between kernel probes and userspace agent

#![cfg_attr(not(feature = "userspace"), no_std)]

/// Simple packet event (legacy, kept for backward compatibility)
#[repr(C)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "userspace", derive(PartialEq, Eq))]
pub struct PacketEvent {
    pub timestamp_ns: u64,
    pub packet_len: u32,
    pub _padding: u32,
}

/// Network flow event with full 5-tuple and container identification
///
/// Layout (32 bytes total, 8-byte aligned):
/// - timestamp_ns: Kernel timestamp in nanoseconds
/// - cgroup_id: Container cgroup ID for pod correlation (0 for TC classifiers)
/// - src_ip: Source IPv4 address (first octet in LSB, as read from TC classifier)
/// - dst_ip: Destination IPv4 address (first octet in LSB, as read from TC classifier)
/// - src_port: Source port (host byte order)
/// - dst_port: Destination port (host byte order)
/// - protocol: IP protocol (6=TCP, 17=UDP, 1=ICMP)
/// - direction: Traffic direction (0=ingress, 1=egress)
/// - packet_len: Packet size in bytes
///
/// Note: IP addresses are stored with first octet in LSB position. For example,
/// 10.0.0.5 is stored as 0x0500000A. Use `from_le_bytes` when parsing IP strings.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "userspace", derive(PartialEq, Eq))]
pub struct NetworkFlowEvent {
    pub timestamp_ns: u64,
    pub cgroup_id: u64,
    pub src_ip: u32,
    pub dst_ip: u32,
    pub src_port: u16,
    pub dst_port: u16,
    pub protocol: u8,
    pub direction: u8,
    pub packet_len: u16,
}

/// Traffic direction constants
pub mod direction {
    pub const INGRESS: u8 = 0;
    pub const EGRESS: u8 = 1;
}

/// IP protocol constants
pub mod protocol {
    pub const ICMP: u8 = 1;
    pub const TCP: u8 = 6;
    pub const UDP: u8 = 17;
}

#[cfg(feature = "userspace")]
const _: () = {
    assert!(
        core::mem::size_of::<PacketEvent>() == 16,
        "PacketEvent must be exactly 16 bytes"
    );
    assert!(
        core::mem::align_of::<PacketEvent>() == 8,
        "PacketEvent must be 8-byte aligned"
    );
};

#[cfg(feature = "userspace")]
const _: () = {
    assert!(
        core::mem::size_of::<NetworkFlowEvent>() == 32,
        "NetworkFlowEvent must be exactly 32 bytes"
    );
    assert!(
        core::mem::align_of::<NetworkFlowEvent>() == 8,
        "NetworkFlowEvent must be 8-byte aligned"
    );
};
