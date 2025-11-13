//! Shared types between eBPF (kernel) and userspace
//!
//! This crate defines event structures that must be:
//! - `#[repr(C)]` for stable memory layout
//! - `no_std` compatible for eBPF
//! - Shared between kernel probes and userspace agent
//!
//! Event types will be added in Phase 1.

#![cfg_attr(not(feature = "userspace"), no_std)]
