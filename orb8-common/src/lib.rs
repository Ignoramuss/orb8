//! Shared types between eBPF (kernel) and userspace
//!
//! This crate defines event structures that must be:
//! - `#[repr(C)]` for stable memory layout
//! - `no_std` compatible for eBPF
//! - Shared between kernel probes and userspace agent

#![cfg_attr(not(feature = "userspace"), no_std)]

#[repr(C)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "userspace", derive(PartialEq, Eq))]
pub struct PacketEvent {
    pub timestamp_ns: u64,
    pub packet_len: u32,
    pub _padding: u32,
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
