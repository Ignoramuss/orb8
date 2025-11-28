//! Network probe that sends packet events via ring buffer
//!
//! This probe:
//! - Attaches as tc classifier on network interfaces
//! - Captures packet metadata (timestamp, length)
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
use orb8_common::PacketEvent;

/// Ring buffer size in bytes. 256KB provides ~16K events before dropping.
/// For production with high packet rates, consider increasing to 1MB or more.
const RING_BUF_SIZE: u32 = 256 * 1024;

#[map]
static EVENTS: RingBuf = RingBuf::with_byte_size(RING_BUF_SIZE, 0);

#[classifier]
pub fn network_probe(ctx: TcContext) -> i32 {
    match try_network_probe(&ctx) {
        Ok(ret) => ret,
        Err(_) => TC_ACT_OK,
    }
}

fn try_network_probe(ctx: &TcContext) -> Result<i32, ()> {
    // SAFETY: bpf_ktime_get_ns is always safe to call from eBPF context
    let timestamp_ns = unsafe { bpf_ktime_get_ns() };

    if let Some(mut entry) = EVENTS.reserve::<PacketEvent>(0) {
        let event = PacketEvent {
            timestamp_ns,
            packet_len: ctx.len(),
            _padding: 0,
        };
        entry.write(event);
        entry.submit(0);
    }
    // Note: If reserve() fails (ring buffer full), event is dropped silently.
    // Consider adding per-CPU dropped event counter for production monitoring.

    Ok(TC_ACT_OK)
}

#[cfg(not(test))]
#[cfg(target_arch = "bpf")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
