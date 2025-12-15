mod v1;

use axum::http::StatusCode;
use axum::{Router, routing::get};
use shared::{init_tracing_and_oltp, initialize_db, load_config};
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(debug_assertions)]
    {
        specta_typescript::Typescript::default()
            .bigint(specta_typescript::BigIntExportBehavior::Number)
            .export_to("./bindings.ts", &specta::export())?;
        return Ok(());
    }
    let (tracer_provider, meter_provider) = init_tracing_and_oltp("data_api")?;
    let config = load_config()?;
    let pool = initialize_db(&config.postgres, false).await?;

    let app = Router::new()
        .route("/health", get(|| async { StatusCode::OK }))
        .nest("/v1", v1::router(pool))
        .layer(CompressionLayer::new())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );

    const LISTEN_ADDR: &str = "0.0.0.0:8080";
    info!("starting server at {LISTEN_ADDR}");
    let listener = tokio::net::TcpListener::bind(LISTEN_ADDR).await?;

    let res = axum::serve(listener, app)
        .with_graceful_shutdown(shared::shutdown_listener(None))
        .await;

    if let Err(e) = res.as_ref() {
        warn!(name: "axum.shutdown", error = ?e, "error while shutting down axum");
    }

    if let Err(e) = tracer_provider.shutdown() {
        eprintln!("failed to shut down tracer provider: {e:?}");
    }

    if let Err(e) = meter_provider.shutdown() {
        eprintln!("failed to shut down tracer provider: {e:?}");
    }

    res.map_err(|e| Box::new(e).into())
}
