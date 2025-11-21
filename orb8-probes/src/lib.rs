//! eBPF probes for orb8
//!
//! This crate contains eBPF programs that run in kernel space to capture:
//! - Network flows (tc hook)
//! - System calls (tracepoint)
//! - GPU telemetry (kprobe/uprobe)
//!
//! Probes extract cgroup IDs to enable container/pod identification.
//!
//! eBPF probe implementations are in src/bin/

#![cfg_attr(not(test), no_std)]

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
