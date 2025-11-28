use anyhow::anyhow;
use aya_build::{Package, Toolchain};
use std::env;

fn main() -> anyhow::Result<()> {
    // Skip eBPF build if we're already building for the eBPF target
    if env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default() == "bpf" {
        return Ok(());
    }

    // Skip eBPF build on non-Linux platforms
    if env::consts::OS != "linux" {
        println!(
            "cargo:warning=eBPF compilation skipped on {}. Use Lima VM for eBPF builds.",
            env::consts::OS
        );
        return Ok(());
    }

    // Skip eBPF build in CI (no bpf-linker available)
    if env::var("CI").is_ok() {
        println!("cargo:warning=eBPF compilation skipped in CI. Use dedicated eBPF build job.");
        return Ok(());
    }

    let ebpf_package = Package {
        name: "orb8-probes",
        root_dir: "../orb8-probes",
        no_default_features: false,
        features: &[],
    };

    aya_build::build_ebpf([ebpf_package], Toolchain::Nightly)?;

    let out_dir = env::var("OUT_DIR")?;
    let probe_path = format!("{}/network_probe", out_dir);
    if !std::path::Path::new(&probe_path).exists() {
        return Err(anyhow!(
            "eBPF probe compilation failed: {} not found",
            probe_path
        ));
    }

    Ok(())
}
