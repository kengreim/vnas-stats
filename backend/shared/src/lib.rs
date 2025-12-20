pub mod vatsim;
pub mod vnas;

use crate::error::InitializationError::MissingEnvVar;
use crate::error::{ConfigError, InitializationError};
use crate::vatsim::OauthEnvironment;
use figment::Figment;
use figment::providers::{Env, Format, Toml};
use opentelemetry::global;
use opentelemetry::trace::TracerProvider;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{LogExporterBuilder, MetricExporterBuilder, WithTonicConfig};
use opentelemetry_resource_detectors::ProcessResourceDetector;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::resource::{
    EnvResourceDetector, ResourceDetector, SdkProvidedResourceDetector,
};
use opentelemetry_sdk::trace::SdkTracerProvider;
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use std::env;
use tokio::signal;
use tokio_util::sync::CancellationToken;
use tracing::{info, instrument};
use tracing_subscriber::fmt::Layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, registry};

pub const DATAFEED_QUEUE_NAME: &str = "vnas_stats";
pub const ENV_VAR_PREFIX: &str = "VNAS_STATS__";
pub const SETTINGS_FILE: &str = "Settings.toml";

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub postgres: PostgresConfig,
    pub fetcher: Option<FetcherConfig>,
    pub oauth: Option<OAuthConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PostgresConfig {
    pub connection_string: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FetcherConfig {
    pub interval_seconds: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OAuthConfig {
    pub client_id: i32,
    pub client_secret: String,
    pub redirect_url: String,
    pub environment: OauthEnvironment,
    pub frontend_login_success_url: String,
}

pub fn load_config() -> Result<Config, ConfigError> {
    Ok(Figment::new()
        .merge(Toml::file(SETTINGS_FILE))
        .merge(Env::prefixed(ENV_VAR_PREFIX).split("__"))
        .extract::<Config>()?)
}

pub mod error {
    use thiserror::Error;
    use tracing::dispatcher::SetGlobalDefaultError;

    #[derive(Debug, Error)]
    pub enum ConfigError {
        #[error("failed to load configuration: {0}")]
        Figment(#[from] figment::Error),
    }

    #[derive(Debug, Error)]
    pub enum InitializationError {
        #[error(transparent)]
        Tracing(#[from] SetGlobalDefaultError),
        #[error(transparent)]
        Config(#[from] ConfigError),
        #[error(transparent)]
        Migration(#[from] sqlx::migrate::MigrateError),
        #[error(transparent)]
        Db(#[from] sqlx::Error),
        #[error("missing environment variable {0}")]
        MissingEnvVar(String),
    }
}

#[instrument]
pub async fn initialize_db(
    pg_config: &PostgresConfig,
    migrate: bool,
) -> Result<Pool<Postgres>, InitializationError> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&pg_config.connection_string)
        .await?;

    info!(name: "db.connected", "db pool created and connected");

    // Run any new migrations
    if migrate {
        sqlx::migrate!("./migrations").run(&pool).await?;
    }

    Ok(pool)
}

pub async fn shutdown_listener(token: Option<CancellationToken>) {
    let ctrl_c = signal::ctrl_c();
    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!(name: "signal.ctrlc.received", "received Ctrl+C signal, shutting down"),
        _ = terminate => info!(name: "signal.sigterm.received", "received SIGTERM signal, shutting down"),
    }

    if let Some(token) = token {
        token.cancel();
    }
}

pub fn init_tracing_and_oltp(
    name: impl ToString,
) -> Result<(SdkTracerProvider, SdkMeterProvider), InitializationError> {
    // OpenTelemetry env vars that should be set at a minimum
    let env_vars = vec![
        "OTEL_SERVICE_NAME",
        "OTEL_EXPORTER_OTLP_ENDPOINT",
        "OTEL_EXPORTER_OTLP_HEADERS",
    ];
    for var in env_vars {
        let _ = env::var(var).map_err(|_| MissingEnvVar(var.to_string()));
    }

    // tracing_opentelemetry setup for spans
    let span_exporter = opentelemetry_otlp::SpanExporterBuilder::default()
        .with_tonic()
        .with_tls_config(tonic::transport::ClientTlsConfig::new().with_native_roots())
        .build()
        .expect("Failed to create OTLP span exporter");

    let detectors: Vec<Box<dyn ResourceDetector>> = vec![
        Box::new(SdkProvidedResourceDetector),
        Box::new(EnvResourceDetector::new()),
        Box::new(ProcessResourceDetector),
    ];
    let resource = Resource::builder().with_detectors(&detectors).build();

    let tracer_provider = SdkTracerProvider::builder()
        .with_resource(resource)
        .with_batch_exporter(span_exporter)
        .build();
    let tracer = tracer_provider.tracer(name.to_string());
    global::set_tracer_provider(tracer_provider.clone());

    let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    // opentelemetry_appender_tracing setup for logs
    let log_exporter = LogExporterBuilder::default()
        .with_tonic()
        .with_tls_config(tonic::transport::ClientTlsConfig::new().with_native_roots())
        .build()
        .expect("Failed to create OTLP log exporter");

    let logger_provider = SdkLoggerProvider::builder()
        .with_batch_exporter(log_exporter)
        .build();

    let otel_log_layer = OpenTelemetryTracingBridge::new(&logger_provider);

    // Setup for OTel Metrics
    let meter_exporter = MetricExporterBuilder::new()
        .with_tonic()
        .with_tls_config(tonic::transport::ClientTlsConfig::new().with_native_roots())
        .build()
        .expect("Failed to create OTLP metric exporter");

    let meter_provider = SdkMeterProvider::builder()
        .with_periodic_exporter(meter_exporter)
        .build();
    global::set_meter_provider(meter_provider.clone());

    // Standard console format and env filter layers
    let fmt_layer = Layer::new()
        .compact()
        .with_file(true)
        .with_line_number(true);

    let env_filter_layer =
        EnvFilter::try_from_default_env().expect("failed to get RUST_LOG from env");

    registry()
        .with(env_filter_layer)
        .with(fmt_layer)
        .with(otel_log_layer)
        .with(telemetry_layer)
        .init();

    Ok((tracer_provider, meter_provider))
}
