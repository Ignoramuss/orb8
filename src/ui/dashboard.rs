use crate::metrics::collector::Metrics;
use crate::Result;

pub struct Dashboard {
    enabled: bool,
}

impl Dashboard {
    pub fn new() -> Self {
        Self { enabled: false }
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub async fn render(&self, _metrics: &Metrics) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        Ok(())
    }
}

impl Default for Dashboard {
    fn default() -> Self {
        Self::new()
    }
}
