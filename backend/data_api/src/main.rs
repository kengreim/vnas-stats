mod v1;

use axum::http::StatusCode;
use axum::{Router, routing::get};
use shared::{initialize_db, load_config};
use tower_http::cors::{Any, CorsLayer};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(debug_assertions)]
    {
        specta_typescript::Typescript::default()
            .bigint(specta_typescript::BigIntExportBehavior::Number)
            .export_to("./bindings.ts", &specta::export())?;
        return Ok(());
    }

    let subscriber = tracing_subscriber::fmt()
        .compact()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let config = load_config()?;
    let pool = initialize_db(&config.postgres).await?;

    let app = Router::new()
        .route("/health", get(|| async { StatusCode::OK }))
        .nest("/v1", v1::router(pool))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );

    const LISTEN_ADDR: &str = "0.0.0.0:8080";
    info!("starting server at {LISTEN_ADDR}");
    let listener = tokio::net::TcpListener::bind(LISTEN_ADDR).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shared::shutdown_listener(None))
        .await?;

    Ok(())
}
