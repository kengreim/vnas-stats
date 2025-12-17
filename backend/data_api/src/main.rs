mod state;
mod v1;

use crate::state::{Db, HttpClients};
use anyhow::anyhow;
use axum::http::StatusCode;
use axum::{Router, routing::get};
use oauth2::{AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl, basic::BasicClient};
use shared::vatsim::OauthEndpoints;
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

    let session_store = PostgresStore::new(pool.clone())
        .with_schema_name("data_api")
        .map_err(|e| anyhow!("{e}"))?
        .with_table_name("sessions")
        .map_err(|e| anyhow!("{e}"))?;

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

    let Some(oauth_config) = config.oauth else {
        error!(name: "config.oauth.loaded", "OAuth client not configured, required for data_api");
        anyhow::bail!("OAuth client not configured, required for data_api");
    };

    let endpoints = OauthEndpoints::from(oauth_config.environment);

    let oauth_client = BasicClient::new(ClientId::new(oauth_config.client_id.to_string()))
        .set_client_secret(ClientSecret::new(oauth_config.client_secret))
        .set_auth_uri(AuthUrl::new(endpoints.auth_url)?)
        .set_token_uri(TokenUrl::new(endpoints.token_url)?)
        .set_redirect_uri(RedirectUrl::new(oauth_config.redirect_url)?);

    let standard_http_client = reqwest::ClientBuilder::new().build()?;
    let no_redirect_http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let state = state::AppState {
        db: Db { pool },
        oauth_client,
        oauth_env: oauth_config.environment,
        http_clients: HttpClients {
            standard: standard_http_client,
            no_redirect: no_redirect_http_client,
        },
    };

    let app = Router::new()
        .route("/health", get(|| async { StatusCode::OK }))
        .nest("/v1", v1::router(state.clone()))
        .layer(session_layer)
        .layer(CompressionLayer::new())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
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
