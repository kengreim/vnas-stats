mod state;
mod v1;

use crate::state::{Db, HttpClients};
use axum::http::StatusCode;
use axum::{Router, routing::get};
use oauth2::{AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl, basic::BasicClient};
use shared::{init_tracing_and_oltp, initialize_db, load_config};
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_sessions::{ExpiredDeletion, Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::PostgresStore;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // #[cfg(debug_assertions)]
    // {
    //     specta_typescript::Typescript::default()
    //         .bigint(specta_typescript::BigIntExportBehavior::Number)
    //         .export_to("./bindings.ts", &specta::export())?;
    //     return Ok(());
    // }
    let (tracer_provider, meter_provider) = init_tracing_and_oltp("data_api")?;
    let config = load_config()?;
    info!(name: "config.loaded", config = ?config, "config loaded");
    let pool = initialize_db(&config.postgres, false).await?;

    let session_store = PostgresStore::new(pool.clone());

    let cleanup_handle = tokio::spawn(
        session_store
            .clone() // Clone store if it needs to be moved/shared
            .continuously_delete_expired(tokio::time::Duration::from_secs(60)), // Clean every 60s
    );

    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false)
        .with_same_site(tower_sessions::cookie::SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(
            tower_sessions::cookie::time::Duration::days(7),
        ));

    let oauth_client = if let Some(oauth_config) = config.oauth {
        BasicClient::new(ClientId::new(oauth_config.client_id))
            .set_client_secret(ClientSecret::new(oauth_config.client_secret))
            .set_auth_uri(AuthUrl::new(oauth_config.auth_url)?)
            .set_token_uri(TokenUrl::new(oauth_config.token_url)?)
            .set_redirect_uri(RedirectUrl::new(oauth_config.redirect_url)?)
    } else {
        error!(name: "config.oauth.loaded", "OAuth client not configured, required for data_api");
        anyhow::bail!("OAuth client not configured, required for data_api");
    };

    let standard_http_client = reqwest::ClientBuilder::new().build()?;
    let no_redirect_http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let state = state::AppState {
        db: Db { pool },
        oauth_client,
        http_clients: HttpClients {
            standard: standard_http_client,
            no_redirect: no_redirect_http_client,
        },
    };

    let app = Router::new()
        .route("/health", get(|| async { StatusCode::OK }))
        .nest("/v1", v1::router())
        .layer(session_layer)
        .layer(CompressionLayer::new())
        .layer(
            CorsLayer::new()
                .allow_origin(["http://localhost:5173".parse().unwrap()])
                .allow_methods(Any)
                .allow_headers(Any)
                .allow_credentials(true),
        )
        .with_state(state);

    const LISTEN_ADDR: &str = "0.0.0.0:8080";
    info!("starting server at {LISTEN_ADDR}");
    let listener = tokio::net::TcpListener::bind(LISTEN_ADDR).await?;

    let res = axum::serve(listener, app)
        .with_graceful_shutdown(shared::shutdown_listener(None))
        .await;

    if let Err(e) = res.as_ref() {
        warn!(name: "axum.shutdown", error = ?e, "error while shutting down axum");
    }

    info!(name:"sessions.shutdown", "cleaning up session cleanup task");
    cleanup_handle.abort();
    if let Err(e) = cleanup_handle.await {
        warn!(name: "sessions.shutdown", error = ?e, "failed to end session cleanup task");
    }

    if let Err(e) = tracer_provider.shutdown() {
        eprintln!("failed to shut down tracer provider: {e:?}");
    }

    if let Err(e) = meter_provider.shutdown() {
        eprintln!("failed to shut down tracer provider: {e:?}");
    }

    Ok(res?)
}
