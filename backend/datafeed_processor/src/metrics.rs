use opentelemetry::global;
use opentelemetry::metrics::{Counter, Gauge};

#[derive(Clone, Default)]
pub struct Metrics {
    pub datafeeds: DatafeedsMetrics,
    pub sessions: SessionsMetrics,
    pub active: ActiveMetrics,
}

#[derive(Clone)]
pub struct DatafeedsMetrics {
    pub processed: Counter<u64>,
    pub bytes_uncompressed: Counter<u64>,
    pub bytes_compressed: Counter<u64>,
}

#[derive(Clone)]
pub struct SessionsMetrics {
    pub controller_opened: Counter<u64>,
    pub callsign_opened: Counter<u64>,
    pub position_opened: Counter<u64>,
}

#[derive(Clone)]
pub struct ActiveMetrics {
    pub controllers: Gauge<u64>,
    pub callsigns: Gauge<u64>,
    pub positions: Gauge<u64>,
}

impl Default for DatafeedsMetrics {
    fn default() -> Self {
        let meter = global::meter("datafeed_processor");
        let processed = meter.u64_counter("datafeeds.processed").build();
        let bytes_uncompressed = meter
            .u64_counter("datafeeds.processed.bytes.uncompressed")
            .with_unit("B")
            .build();
        let bytes_compressed = meter
            .u64_counter("datafeeds.processed.bytes.compressed")
            .with_unit("B")
            .build();

        Self {
            processed,
            bytes_uncompressed,
            bytes_compressed,
        }
    }
}

impl Default for SessionsMetrics {
    fn default() -> Self {
        let meter = global::meter("datafeed_processor");
        let controller_opened = meter.u64_counter("sessions.controller.opened").build();
        let callsign_opened = meter.u64_counter("sessions.callsign.opened").build();
        let position_opened = meter.u64_counter("sessions.position.opened").build();

        Self {
            controller_opened,
            callsign_opened,
            position_opened,
        }
    }
}

impl Default for ActiveMetrics {
    fn default() -> Self {
        let meter = global::meter("datafeed_processor");
        let controllers = meter.u64_gauge("sessions.controller.active").build();
        let callsigns = meter.u64_gauge("sessions.callsign.active").build();
        let positions = meter.u64_gauge("sessions.position.active").build();

        Self {
            controllers,
            callsigns,
            positions,
        }
    }
}
