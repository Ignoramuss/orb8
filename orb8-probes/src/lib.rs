//! eBPF probes for orb8
//!
//! This crate contains eBPF programs that run in kernel space to capture:
//! - Network flows (tc hook)
//! - System calls (tracepoint)
//! - GPU telemetry (kprobe/uprobe)
//!
//! Probes extract cgroup IDs to enable container/pod identification.
//!
//! Implementation will be added in Phase 1.

#![no_std]
#![no_main]

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
