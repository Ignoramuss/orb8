use crate::ebpf::events::Event;
use crate::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Default)]
pub struct Metrics {
    pub network_packets_total: u64,
    pub network_bytes_total: u64,
    pub syscalls_total: u64,
    pub gpu_allocations_total: u64,
    pub gpu_memory_allocated: u64,
}

pub struct MetricsCollector {
    metrics: Arc<RwLock<Metrics>>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(Metrics::default())),
        }
    }

    pub async fn process_event(&self, event: Event) -> Result<()> {
        let mut metrics = self.metrics.write().await;

        match event {
            Event::Network(net_event) => {
                metrics.network_packets_total += 1;
                metrics.network_bytes_total += net_event.bytes;
            }
            Event::Syscall(_) => {
                metrics.syscalls_total += 1;
            }
            Event::Gpu(gpu_event) => {
                use crate::ebpf::events::GpuEventType;
                match gpu_event.event_type {
                    GpuEventType::Alloc => {
                        metrics.gpu_allocations_total += 1;
                        metrics.gpu_memory_allocated += gpu_event.size;
                    }
                    GpuEventType::Free => {
                        if metrics.gpu_memory_allocated >= gpu_event.size {
                            metrics.gpu_memory_allocated -= gpu_event.size;
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    pub async fn get_metrics(&self) -> Metrics {
        self.metrics.read().await.clone()
    }

    pub fn metrics_ref(&self) -> Arc<RwLock<Metrics>> {
        self.metrics.clone()
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}
