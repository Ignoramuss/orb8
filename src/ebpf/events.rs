use bytes::Bytes;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub enum Event {
    Network(NetworkEvent),
    Syscall(SyscallEvent),
    Gpu(GpuEvent),
}

#[derive(Debug, Clone)]
pub struct NetworkEvent {
    pub timestamp: SystemTime,
    pub src_ip: String,
    pub dst_ip: String,
    pub src_port: u16,
    pub dst_port: u16,
    pub protocol: Protocol,
    pub bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Tcp,
    Udp,
    Icmp,
    Other(u8),
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::Tcp => write!(f, "TCP"),
            Protocol::Udp => write!(f, "UDP"),
            Protocol::Icmp => write!(f, "ICMP"),
            Protocol::Other(proto) => write!(f, "Protocol({})", proto),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SyscallEvent {
    pub timestamp: SystemTime,
    pub pid: u32,
    pub syscall_id: u64,
    pub syscall_name: String,
}

#[derive(Debug, Clone)]
pub struct GpuEvent {
    pub timestamp: SystemTime,
    pub event_type: GpuEventType,
    pub size: u64,
    pub device_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuEventType {
    Alloc,
    Free,
    KernelLaunch,
    MemoryCopy,
}

pub struct EventProcessor {
    buffer: Vec<Event>,
}

impl EventProcessor {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    pub fn process_raw_event(&mut self, _data: Bytes) -> Option<Event> {
        None
    }

    pub fn flush(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.buffer)
    }
}

impl Default for EventProcessor {
    fn default() -> Self {
        Self::new()
    }
}
