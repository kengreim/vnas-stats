mod model;

use chrono::{DateTime, Utc};
use model::{FlatFacility, FlatPosition};
use reqwest::Client;
use shared::PostgresConfig;
use shared::error::InitializationError;
use shared::load_config;
use shared::vnas::api::minimal::Facility as MinimalFacility;
use shared::vnas::api::minimal::{ArtccRoot as MinimalArtccRoot, ArtccRoot};
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres, Row};
use thiserror::Error;
use tracing::{debug, error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let subscriber = tracing_subscriber::fmt()
        .compact()
        .with_file(true)
        .with_line_number(true)
        .with_env_filter(EnvFilter::from_default_env())
        .finish();

    tracing::subscriber::set_global_default(subscriber).map_err(InitializationError::from)?;

    let config = load_config().map_err(InitializationError::from)?;
    info!(config = ?config, "config loaded");
    let db_pool = initialize_db(&config.postgres).await?;
    let client = Client::new();

    let res = fetch_and_process(&client, &db_pool).await;
    match res {
        Ok(()) => info!("ARTCC data sync was successful"),
        Err(ref e) => error!(error = ?e, "failed to sync ARTCC data"),
    }

    res
}

async fn initialize_db(pg_config: &PostgresConfig) -> Result<Pool<Postgres>, AppError> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&pg_config.connection_string)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

async fn fetch_and_process(client: &Client, pool: &Pool<Postgres>) -> Result<(), AppError> {
    let artccs = fetch_artccs(client).await?;
    process_artccs(pool, artccs).await?;
    Ok(())
}

async fn fetch_artccs(client: &Client) -> Result<Vec<MinimalArtccRoot>, AppError> {
    debug!("fetching all ARTCCs data from vNAS API");
    let resp = client
        .get(shared::vnas::api::ALL_ARTCCS_ENDPOINT)
        .send()
        .await?
        .error_for_status()?
        .json::<Vec<MinimalArtccRoot>>()
        .await?;
    debug!(len = resp.len(), "fetched ARTCCs from vNAS API");

    Ok(resp)
}

async fn find_artccs_to_update<'a>(
    pool: &'a Pool<Postgres>,
    artccs: &'a [MinimalArtccRoot],
) -> Result<Vec<&'a ArtccRoot>, AppError> {
    let existing_artccs = sqlx::query(
        r#"
        SELECT id, last_updated_at
        FROM facilities
        WHERE facility_type = 'Artcc'
        "#,
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| {
        let id: String = row.get("id");
        let ts: DateTime<Utc> = row.get("last_updated_at");
        (id, ts)
    })
    .collect::<std::collections::HashMap<_, _>>();

    Ok(artccs
        .iter()
        .filter(move |a| match existing_artccs.get(&a.id) {
            None => true,
            Some(stored_last_updated_at) => a.last_updated_at > *stored_last_updated_at,
        })
        .collect())
}

async fn process_artccs(
    pool: &Pool<Postgres>,
    artccs: Vec<MinimalArtccRoot>,
) -> Result<(), AppError> {
    let mut facilities = Vec::new();
    let mut positions = Vec::new();

    let artccs_to_update = find_artccs_to_update(pool, &artccs).await?;
    let updated_artcc_ids = artccs_to_update
        .iter()
        .map(|a| a.id.to_owned())
        .collect::<Vec<_>>();
    if updated_artcc_ids.is_empty() {
        info!("found no ARTCCs to update");
    } else {
        info!(ids = ?updated_artcc_ids, "found ARTCCs to update");
    }

    for artcc in artccs_to_update {
        collect_facility_tree(
            &artcc.facility,
            None,
            artcc.last_updated_at,
            &artcc.id,
            &mut facilities,
            &mut positions,
        );
    }

    let mut tx = pool.begin().await?;

    for facility in &facilities {
        sqlx::query(
            r#"
            INSERT INTO facilities (
                id,
                root_artcc_id,
                parent_id,
                name,
                facility_type,
                last_updated_at,
                first_seen,
                is_active
            )
            VALUES ($1, $2, $3, $4, $5::facility_type, $6, $6, TRUE)
            ON CONFLICT (id) DO UPDATE
            SET parent_id = EXCLUDED.parent_id,
                name = EXCLUDED.name,
                facility_type = EXCLUDED.facility_type,
                last_updated_at = EXCLUDED.last_updated_at,
                first_seen = LEAST(facilities.first_seen, EXCLUDED.first_seen),
                is_active = TRUE
            "#,
        )
        .bind(&facility.id)
        .bind(&facility.root_artcc_id)
        .bind(&facility.parent_id)
        .bind(&facility.name)
        .bind(&facility.facility_type)
        .bind(facility.last_updated_at)
        .execute(&mut *tx)
        .await?;
    }

    // Mark facilities stale relative to their root ARTCC's last_updated_at as inactive.
    if !updated_artcc_ids.is_empty() {
        let processed_artcc_ids_refs: Vec<&str> =
            updated_artcc_ids.iter().map(String::as_str).collect();
        let n = sqlx::query(
            r#"
            UPDATE facilities f
            SET is_active = FALSE
            WHERE f.root_artcc_id = ANY($1)
              AND f.is_active = TRUE
              AND f.last_updated_at < (
                    SELECT r.last_updated_at FROM facilities r
                    WHERE r.id = f.root_artcc_id
                )
            "#,
        )
        .bind(&processed_artcc_ids_refs)
        .execute(&mut *tx)
        .await?;
        info!(n = n.rows_affected(), "marked facilities inactive");
    }

    for position in &positions {
        sqlx::query(
            r#"
            INSERT INTO facility_positions (
                id,
                facility_id,
                name,
                callsign,
            radio_name,
            frequency,
            starred,
            last_updated_at,
            first_seen,
            is_active
        )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8, TRUE)
            ON CONFLICT (id) DO UPDATE
            SET facility_id = EXCLUDED.facility_id,
                name = EXCLUDED.name,
                callsign = EXCLUDED.callsign,
                radio_name = EXCLUDED.radio_name,
                frequency = EXCLUDED.frequency,
                starred = EXCLUDED.starred,
                last_updated_at = EXCLUDED.last_updated_at,
                first_seen = LEAST(facility_positions.first_seen, EXCLUDED.first_seen),
                is_active = TRUE
            "#,
        )
        .bind(&position.id)
        .bind(&position.facility_id)
        .bind(&position.name)
        .bind(&position.callsign)
        .bind(&position.radio_name)
        .bind(position.frequency)
        .bind(position.starred)
        .bind(position.last_updated_at)
        .execute(&mut *tx)
        .await?;
    }

    if !updated_artcc_ids.is_empty() {
        let processed_artcc_ids_refs: Vec<&str> =
            updated_artcc_ids.iter().map(String::as_str).collect();
        let n = sqlx::query(
            r#"
            UPDATE facility_positions p
            SET is_active = FALSE
            WHERE p.is_active = TRUE
              AND EXISTS (
                    SELECT 1
                    FROM facilities f
                    JOIN facilities root ON root.id = f.root_artcc_id
                    WHERE f.id = p.facility_id
                      AND root.id = ANY($1)
                      AND p.last_updated_at < root.last_updated_at
                )
            "#,
        )
        .bind(&processed_artcc_ids_refs)
        .execute(&mut *tx)
        .await?;
        info!(n = n.rows_affected(), "marked positions inactive")
    }

    tx.commit().await?;
    Ok(())
}

fn collect_facility_tree(
    facility: &MinimalFacility,
    parent_id: Option<String>,
    last_updated_at: DateTime<Utc>,
    root_artcc_id: &str,
    facilities: &mut Vec<FlatFacility>,
    positions: &mut Vec<FlatPosition>,
) {
    facilities.push(FlatFacility {
        id: facility.id.clone(),
        root_artcc_id: root_artcc_id.to_string(),
        parent_id,
        name: facility.name.clone(),
        facility_type: facility.type_field.to_string(),
        last_updated_at,
    });

    for position in &facility.positions {
        positions.push(FlatPosition {
            id: position.id.clone(),
            facility_id: facility.id.clone(),
            name: position.name.clone(),
            callsign: Some(position.callsign.clone()),
            radio_name: Some(position.radio_name.clone()),
            frequency: Some(position.frequency),
            starred: position.starred,
            last_updated_at,
        });
    }

    for child in &facility.child_facilities {
        collect_facility_tree(
            child,
            Some(facility.id.clone()),
            last_updated_at,
            root_artcc_id,
            facilities,
            positions,
        );
    }
}

#[derive(Debug, Error)]
enum AppError {
    #[error("initialization error: {0}")]
    Initialization(#[from] InitializationError),
    #[error("network or fetch error: {0}")]
    Fetch(#[from] reqwest::Error),
    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}
