//! Minimal "Hello World" eBPF probe for network traffic
//!
//! This probe demonstrates:
//! - Basic tc (traffic control) classifier attachment
//! - eBPF logging using aya-log-ebpf
//! - Proof that the eBPF toolchain works end-to-end
//!
//! Attaches to loopback interface (lo) for safe testing.

#![no_std]
#![no_main]

use aya_ebpf::{bindings::TC_ACT_OK, macros::classifier, programs::TcContext};
use aya_log_ebpf::info;

#[classifier]
pub fn network_probe(ctx: TcContext) -> i32 {
    match try_network_probe(ctx) {
        Ok(ret) => ret,
        Err(_) => TC_ACT_OK,
    }
}

fn try_network_probe(ctx: TcContext) -> Result<i32, ()> {
    info!(&ctx, "Hello from eBPF! packet_len={}", ctx.len());
    Ok(TC_ACT_OK)
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
