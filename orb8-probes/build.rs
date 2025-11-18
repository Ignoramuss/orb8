use anyhow::{anyhow, Context};

fn main() -> anyhow::Result<()> {
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
