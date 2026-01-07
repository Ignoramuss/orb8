//! Network probe that sends packet events via ring buffer
//!
//! This probe:
//! - Attaches as tc classifier on network interfaces
//! - Captures packet metadata (timestamp, length, 5-tuple)
//! - Extracts cgroup ID for container/pod identification
//! - Sends events to userspace via ring buffer
//!
//! Note: This binary must be built for the bpfel-unknown-none target.
//! On macOS, the build will fail if invoked directly. Use orb8-agent's
//! build.rs which handles cross-compilation automatically.

#![no_std]
#![no_main]

use aya_ebpf::{
    bindings::TC_ACT_OK,
    helpers::bpf_ktime_get_ns,
    macros::{classifier, map},
    maps::RingBuf,
    programs::TcContext,
};
use orb8_common::{direction, protocol, NetworkFlowEvent};

/// Ring buffer size in bytes. 1MB provides ~32K events before dropping.
const RING_BUF_SIZE: u32 = 1024 * 1024;

/// Ethernet header constants
const ETH_HLEN: usize = 14;
const ETH_P_IP: u16 = 0x0800;

/// IP header constants
const IP_HLEN_MIN: usize = 20;

#[map]
static EVENTS: RingBuf = RingBuf::with_byte_size(RING_BUF_SIZE, 0);

#[classifier]
pub fn network_probe(ctx: TcContext) -> i32 {
    match try_network_probe(&ctx, direction::INGRESS) {
        Ok(ret) => ret,
        Err(_) => TC_ACT_OK,
    }
}

#[classifier]
pub fn network_probe_egress(ctx: TcContext) -> i32 {
    match try_network_probe(&ctx, direction::EGRESS) {
        Ok(ret) => ret,
        Err(_) => TC_ACT_OK,
    }
}

/// Safe pointer-at function for reading packet data
#[inline(always)]
unsafe fn ptr_at<T>(ctx: &TcContext, offset: usize) -> Result<*const T, ()> {
    let start = ctx.data();
    let end = ctx.data_end();
    let len = core::mem::size_of::<T>();

    if start + offset + len > end {
        return Err(());
    }

    Ok((start + offset) as *const T)
}

fn try_network_probe(ctx: &TcContext, dir: u8) -> Result<i32, ()> {
    // Get timestamp first (always succeeds)
    let timestamp_ns = unsafe { bpf_ktime_get_ns() };
    // Note: bpf_get_current_cgroup_id() is not available for TC classifiers
    // on some kernels. Set to 0 for now - pod enrichment will use other methods.
    let cgroup_id: u64 = 0;

    // Ensure packet is large enough for Ethernet header
    if ctx.len() < (ETH_HLEN + IP_HLEN_MIN) as u32 {
        return Ok(TC_ACT_OK);
    }

    // Read Ethernet header to check protocol
    // Ethernet header: [dst_mac(6), src_mac(6), ethertype(2)]
    let ethertype_ptr = unsafe { ptr_at::<[u8; 2]>(ctx, 12)? };
    let ethertype = u16::from_be_bytes(unsafe { *ethertype_ptr });

    // Only process IPv4 packets
    if ethertype != ETH_P_IP {
        return Ok(TC_ACT_OK);
    }

    // Read IPv4 header
    // IPv4 header: [version_ihl(1), tos(1), total_len(2), id(2), frag_off(2),
    //               ttl(1), protocol(1), checksum(2), src_ip(4), dst_ip(4)]
    let ip_offset = ETH_HLEN;

    // Read version/IHL byte to get header length
    let version_ihl_ptr = unsafe { ptr_at::<u8>(ctx, ip_offset)? };
    let version_ihl = unsafe { *version_ihl_ptr };
    let ip_header_len = ((version_ihl & 0x0F) as usize) * 4;

    // Validate IP header length
    if ip_header_len < IP_HLEN_MIN {
        return Ok(TC_ACT_OK);
    }

    // Read protocol (offset 9 from IP header start)
    let proto_ptr = unsafe { ptr_at::<u8>(ctx, ip_offset + 9)? };
    let proto = unsafe { *proto_ptr };

    // Read src/dst IP (offsets 12 and 16 from IP header start)
    let src_ip_ptr = unsafe { ptr_at::<u32>(ctx, ip_offset + 12)? };
    let dst_ip_ptr = unsafe { ptr_at::<u32>(ctx, ip_offset + 16)? };
    let src_ip = unsafe { *src_ip_ptr };
    let dst_ip = unsafe { *dst_ip_ptr };

    // Parse transport layer for ports
    let transport_offset = ip_offset + ip_header_len;
    let (src_port, dst_port) = match proto {
        protocol::TCP | protocol::UDP => {
            // TCP/UDP headers both have src_port(2), dst_port(2) at the start
            let sport_ptr = unsafe { ptr_at::<[u8; 2]>(ctx, transport_offset)? };
            let dport_ptr = unsafe { ptr_at::<[u8; 2]>(ctx, transport_offset + 2)? };
            let sport = u16::from_be_bytes(unsafe { *sport_ptr });
            let dport = u16::from_be_bytes(unsafe { *dport_ptr });
            (sport, dport)
        }
        _ => (0, 0), // No ports for ICMP and other protocols
    };

    // Submit event to ring buffer
    if let Some(mut entry) = EVENTS.reserve::<NetworkFlowEvent>(0) {
        let event = NetworkFlowEvent {
            timestamp_ns,
            cgroup_id,
            src_ip,
            dst_ip,
            src_port,
            dst_port,
            protocol: proto,
            direction: dir,
            packet_len: ctx.len() as u16,
        };
        entry.write(event);
        entry.submit(0);
    }

    Ok(TC_ACT_OK)
}

#[cfg(not(test))]
#[cfg(target_arch = "bpf")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
