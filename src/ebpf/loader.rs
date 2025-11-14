use crate::{Orb8Error, Result};
use std::path::PathBuf;
use tracing::{debug, info};

pub struct ProbeLoader {
    probes_dir: PathBuf,
}

impl ProbeLoader {
    pub fn new(probes_dir: PathBuf) -> Self {
        Self { probes_dir }
    }

    pub fn with_default_path() -> Self {
        Self {
            probes_dir: PathBuf::from("/usr/lib/orb8/probes"),
        }
    }

    pub async fn load_probe(&self, name: &str) -> Result<LoadedProbe> {
        debug!("Loading eBPF probe: {}", name);

        let probe_path = self.probes_dir.join(format!("{}.o", name));

        if !probe_path.exists() {
            return Err(Orb8Error::ProgramLoadFailed(format!(
                "Probe file not found: {}",
                probe_path.display()
            )));
        }

        info!("Successfully loaded probe: {}", name);

        Ok(LoadedProbe {
            name: name.to_string(),
            path: probe_path,
            attached: false,
        })
    }
}

#[derive(Debug)]
pub struct LoadedProbe {
    pub name: String,
    pub path: PathBuf,
    pub attached: bool,
}

impl LoadedProbe {
    pub fn attach(&mut self) -> Result<()> {
        debug!("Attaching probe: {}", self.name);
        self.attached = true;
        Ok(())
    }

    pub fn detach(&mut self) -> Result<()> {
        debug!("Detaching probe: {}", self.name);
        self.attached = false;
        Ok(())
    }
}
