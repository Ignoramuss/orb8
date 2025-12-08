//! eBPF probe loader and lifecycle management

use anyhow::{anyhow, Context, Result};
use aya::{
    maps::RingBuf,
    programs::{tc, SchedClassifier, TcAttachType},
    Ebpf,
};
use log::{info, warn};
use orb8_common::NetworkFlowEvent;
use std::mem;
use std::path::Path;

/// Manages eBPF probe lifecycle
pub struct ProbeManager {
    bpf: Ebpf,
}

impl ProbeManager {
    /// Create a new ProbeManager and load the network probe
    pub fn new() -> Result<Self> {
        run_preflight_checks()?;

        info!("Loading network probe...");
        let bpf = load_network_probe()?;

        Ok(Self { bpf })
    }

    /// Attach the network probe to the loopback interface
    pub fn attach_to_loopback(&mut self) -> Result<()> {
        info!("Attaching network probe to loopback interface...");

        let program: &mut SchedClassifier = self
            .bpf
            .program_mut("network_probe")
            .ok_or_else(|| anyhow!("network_probe program not found in eBPF object"))?
            .try_into()?;

        program.load()?;

        // Add clsact qdisc to lo interface (required for TC attachment)
        if let Err(e) = tc::qdisc_add_clsact("lo") {
            warn!("Failed to add clsact qdisc (may already exist): {}", e);
        }

        program
            .attach("lo", TcAttachType::Ingress)
            .context("Failed to attach to loopback interface")?;

        info!("Network probe attached to lo interface");
        Ok(())
    }

    /// Get mutable reference to the Ebpf object for initializing the EbpfLogger.
    /// This is required to set up log forwarding from eBPF to userspace.
    pub fn bpf_mut(&mut self) -> &mut Ebpf {
        &mut self.bpf
    }

    /// Get the events ring buffer for polling packet events
    pub fn events_ring_buf(&mut self) -> Result<RingBuf<&mut aya::maps::MapData>> {
        // Collect map names first to avoid borrow conflict in error path
        let available_maps: Vec<_> = self.bpf.maps().map(|(name, _)| name.to_string()).collect();
        let map = self.bpf.map_mut("EVENTS").ok_or_else(|| {
            anyhow!(
                "EVENTS map not found in eBPF object. Available maps: {:?}",
                available_maps
            )
        })?;
        RingBuf::try_from(map).context("Failed to create RingBuf from EVENTS map")
    }

    /// Detach and unload all probes
    pub fn unload(self) {
        info!("Unloading eBPF probes...");
        drop(self.bpf);
        info!("Probes unloaded");
    }
}

/// Poll events from the ring buffer
pub fn poll_events(ring_buf: &mut RingBuf<&mut aya::maps::MapData>) -> Vec<NetworkFlowEvent> {
    const MAX_BATCH_SIZE: usize = 1024;
    let mut events = Vec::new();

    while let Some(item) = ring_buf.next() {
        if events.len() >= MAX_BATCH_SIZE {
            warn!("Hit maximum batch size ({}), stopping poll", MAX_BATCH_SIZE);
            break;
        }

        let expected_size = mem::size_of::<NetworkFlowEvent>();
        if item.len() == expected_size {
            let event: NetworkFlowEvent =
                unsafe { std::ptr::read_unaligned(item.as_ptr() as *const NetworkFlowEvent) };
            events.push(event);
        } else {
            warn!(
                "Malformed event: expected {} bytes, got {} bytes - skipping",
                expected_size,
                item.len()
            );
        }
    }
    events
}

/// Load the network probe eBPF program
fn load_network_probe() -> Result<Ebpf> {
    let bpf = Ebpf::load(aya::include_bytes_aligned!(concat!(
        env!("OUT_DIR"),
        "/network_probe"
    )))
    .context("Failed to load eBPF program")?;

    Ok(bpf)
}

/// Run pre-flight checks to validate the system can run eBPF programs
fn run_preflight_checks() -> Result<()> {
    info!("Running pre-flight checks...");

    check_kernel_version()?;
    check_btf()?;
    check_capabilities()?;

    info!("Pre-flight checks passed");
    Ok(())
}

/// Check if kernel version is >= 5.8
fn check_kernel_version() -> Result<()> {
    let output = std::process::Command::new("uname")
        .arg("-r")
        .output()
        .context("Failed to get kernel version")?;

    let version_str = String::from_utf8(output.stdout)?;
    let parts: Vec<&str> = version_str.split('.').collect();

    if parts.len() < 2 {
        return Err(anyhow!("Could not parse kernel version: {}", version_str));
    }

    let major: u32 = parts[0]
        .trim()
        .parse()
        .context("Invalid kernel major version")?;

    let minor_str = parts[1].split('-').next().unwrap_or(parts[1]);
    let minor: u32 = minor_str
        .trim()
        .parse()
        .context("Invalid kernel minor version")?;

    if major < 5 || (major == 5 && minor < 8) {
        return Err(anyhow!(
            "Kernel {} is too old. eBPF requires kernel 5.8+ (5.15+ recommended)",
            version_str.trim()
        ));
    }

    info!("Kernel version: {} (supported)", version_str.trim());
    Ok(())
}

/// Check if BTF (BPF Type Format) is available
fn check_btf() -> Result<()> {
    let btf_path = Path::new("/sys/kernel/btf/vmlinux");

    if !btf_path.exists() {
        warn!("BTF not found at /sys/kernel/btf/vmlinux");
        warn!("Some eBPF features may not work. Consider rebuilding kernel with CONFIG_DEBUG_INFO_BTF=y");
        return Ok(());
    }

    info!("BTF available");
    Ok(())
}

/// Check if process has necessary capabilities to load eBPF programs
fn check_capabilities() -> Result<()> {
    let euid = unsafe { libc::geteuid() };

    if euid != 0 {
        warn!("Not running as root (euid={}). Ensure CAP_BPF, CAP_NET_ADMIN, and CAP_SYS_ADMIN capabilities are granted.", euid);
    } else {
        info!("Running with root privileges");
    }

    Ok(())
}
