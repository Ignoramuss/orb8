use anyhow::{anyhow, Context};
use std::env;

fn main() -> anyhow::Result<()> {
    // Skip eBPF build if we're already building for the eBPF target
    // This prevents infinite recursion
    if env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default() == "bpf" {
        return Ok(());
    }

    // Skip eBPF build on macOS (Darwin) - eBPF requires Linux
    // Use Lima VM for actual eBPF compilation on macOS
    if env::consts::OS != "linux" {
        println!(
            "cargo:warning=eBPF compilation skipped on {}. Use Lima VM for eBPF builds.",
            env::consts::OS
        );
        return Ok(());
    }

    let cargo_metadata::Metadata { packages, .. } =
        aya_build::cargo_metadata::MetadataCommand::new()
            .no_deps()
            .exec()
            .context("MetadataCommand::exec")?;

    let ebpf_package = packages
        .into_iter()
        .find(|pkg| pkg.name == "orb8-probes")
        .ok_or_else(|| anyhow!("orb8-probes package not found"))?;

    aya_build::build_ebpf([ebpf_package])
}
