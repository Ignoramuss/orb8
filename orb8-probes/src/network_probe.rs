//! Network probe that sends packet events via ring buffer
//!
//! This probe:
//! - Attaches as tc classifier on network interfaces
//! - Captures packet metadata (timestamp, length)
//! - Sends events to userspace via ring buffer

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

#[map]
static EVENTS: RingBuf = RingBuf::with_byte_size(256 * 1024, 0);

#[classifier]
pub fn network_probe(ctx: TcContext) -> i32 {
    match try_network_probe(&ctx) {
        Ok(ret) => ret,
        Err(_) => TC_ACT_OK,
    }
}

fn try_network_probe(ctx: &TcContext) -> Result<i32, ()> {
    let timestamp_ns = unsafe { bpf_ktime_get_ns() };

    if let Some(mut entry) = EVENTS.reserve::<PacketEvent>(0) {
        let event = PacketEvent {
            timestamp_ns,
            packet_len: ctx.len(),
            _padding: 0,
        };
        unsafe { entry.write(event) };
        entry.submit(0);
    }

    Ok(TC_ACT_OK)
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
