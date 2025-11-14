use crate::metrics::collector::{Metrics, MetricsCollector};
use crate::Result;
use tracing::info;

pub struct PrometheusExporter {
    collector: MetricsCollector,
    port: u16,
}

impl PrometheusExporter {
    pub fn new(collector: MetricsCollector, port: u16) -> Self {
        Self { collector, port }
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting Prometheus exporter on port {}", self.port);

        Ok(())
    }

    pub async fn format_current_metrics(&self) -> String {
        let metrics = self.collector.get_metrics().await;
        self.format_metrics(&metrics)
    }

    pub fn format_metrics(&self, metrics: &Metrics) -> String {
        format!(
            "# HELP orb8_network_packets_total Total network packets observed\n\
             # TYPE orb8_network_packets_total counter\n\
             orb8_network_packets_total {}\n\
             \n\
             # HELP orb8_network_bytes_total Total network bytes observed\n\
             # TYPE orb8_network_bytes_total counter\n\
             orb8_network_bytes_total {}\n\
             \n\
             # HELP orb8_syscalls_total Total syscalls observed\n\
             # TYPE orb8_syscalls_total counter\n\
             orb8_syscalls_total {}\n\
             \n\
             # HELP orb8_gpu_allocations_total Total GPU memory allocations\n\
             # TYPE orb8_gpu_allocations_total counter\n\
             orb8_gpu_allocations_total {}\n\
             \n\
             # HELP orb8_gpu_memory_allocated_bytes Currently allocated GPU memory\n\
             # TYPE orb8_gpu_memory_allocated_bytes gauge\n\
             orb8_gpu_memory_allocated_bytes {}\n",
            metrics.network_packets_total,
            metrics.network_bytes_total,
            metrics.syscalls_total,
            metrics.gpu_allocations_total,
            metrics.gpu_memory_allocated,
        )
    }
}
