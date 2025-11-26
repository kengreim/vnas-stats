use crate::database::queries::{
    QueryError, complete_callsign_sessions, complete_position_sessions,
    get_active_callsign_sessions, get_active_controller_session_keys, get_active_position_sessions,
    get_or_create_callsign_session, get_or_create_position_session, insert_controller_session,
    update_active_controller_session, update_callsign_session_last_seen,
    update_position_session_last_seen,
};
use chrono::{DateTime, Utc};
use shared::vnas::datafeed::Controller;
use sqlx::{Postgres, Transaction};
use std::collections::{HashMap, HashSet};
use tracing::debug;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ActiveControllerState {
    pub controller_session_id: Uuid,
    pub login_time: DateTime<Utc>,
    pub callsign_session_id: Uuid,
    pub position_id: String,
    pub position_session_id: Uuid,
}

pub struct SessionMaps<'a> {
    pub active_by_cid: &'a mut HashMap<i32, ActiveControllerState>,
    pub callsign_counts: &'a HashMap<Uuid, usize>,
    pub position_counts: &'a HashMap<Uuid, usize>,
    pub active_position_sessions: &'a HashMap<String, Uuid>,
}

pub struct SessionCollections<'a> {
    pub active_callsign_ids: &'a mut HashSet<Uuid>,
    pub active_position_ids: &'a mut HashSet<String>,
    pub extra_close_callsign: &'a mut Vec<Uuid>,
    pub extra_close_positions: &'a mut Vec<Uuid>,
    pub controllers_to_complete: &'a mut Vec<Uuid>,
}

#[derive(Default)]
pub struct ActiveState {
    pub active_by_cid: HashMap<i32, ActiveControllerState>,
    pub callsign_counts: HashMap<Uuid, usize>,
    pub position_counts: HashMap<Uuid, usize>,
    pub active_callsign_sessions: HashSet<Uuid>,
    pub active_position_sessions: HashMap<String, Uuid>,
}

pub async fn load_active_state(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<ActiveState, QueryError> {
    let mut state = ActiveState::default();
    let conn = tx.as_mut();
    let active_controller_sessions = get_active_controller_session_keys(&mut *conn).await?;
    state.active_by_cid = active_controller_sessions
        .iter()
        .map(|session| {
            (
                session.cid,
                ActiveControllerState {
                    controller_session_id: session.id,
                    login_time: session.login_time,
                    callsign_session_id: session.callsign_session_id,
                    position_id: session.primary_position_id.clone(),
                    position_session_id: session.position_session_id,
                },
            )
        })
        .collect();
    state.active_callsign_sessions = get_active_callsign_sessions(&mut *conn)
        .await?
        .into_iter()
        .map(|s| s.id)
        .collect();
    state.active_position_sessions = get_active_position_sessions(&mut *conn)
        .await?
        .into_iter()
        .map(|s| (s.position_id, s.id))
        .collect();

    state.callsign_counts = active_controller_sessions
        .iter()
        .fold(HashMap::new(), |mut acc, s| {
            *acc.entry(s.callsign_session_id).or_insert(0usize) += 1;
            acc
        });
    state.position_counts = active_controller_sessions
        .iter()
        .fold(HashMap::new(), |mut acc, s| {
            *acc.entry(s.position_session_id).or_insert(0usize) += 1;
            acc
        });

    Ok(state)
}

pub async fn handle_active_controller(
    tx: &mut Transaction<'_, Postgres>,
    controller: &Controller,
    cid: i32,
    prefix: &str,
    suffix: &str,
    position_id: &str,
    updated_at: DateTime<Utc>,
    maps: SessionMaps<'_>,
    collections: SessionCollections<'_>,
) -> Result<(), QueryError> {
    let conn = tx.as_mut();
    let SessionMaps {
        active_by_cid,
        callsign_counts,
        position_counts,
        active_position_sessions,
    } = maps;
    let SessionCollections {
        active_callsign_ids,
        active_position_ids,
        extra_close_callsign,
        extra_close_positions,
        controllers_to_complete,
    } = collections;

    if let Some(existing) = active_by_cid.remove(&cid) {
        if existing.login_time == controller.login_time && existing.position_id == position_id {
            update_callsign_session_last_seen(conn, existing.callsign_session_id, updated_at)
                .await?;
            update_position_session_last_seen(conn, existing.position_session_id, updated_at)
                .await?;
            update_active_controller_session(
                conn,
                existing.controller_session_id,
                controller,
                updated_at,
            )
            .await?;
            active_callsign_ids.insert(existing.callsign_session_id);
            active_position_ids.insert(existing.position_id);
        } else {
            controllers_to_complete.push(existing.controller_session_id);
            let callsign_count = callsign_counts
                .get(&existing.callsign_session_id)
                .copied()
                .unwrap_or(0);
            let new_callsign_session_id = if callsign_count <= 1 {
                extra_close_callsign.push(existing.callsign_session_id);
                get_or_create_callsign_session(conn, prefix, suffix, updated_at).await?
            } else {
                update_callsign_session_last_seen(conn, existing.callsign_session_id, updated_at)
                    .await?;
                existing.callsign_session_id
            };
            active_callsign_ids.insert(new_callsign_session_id);

            let position_count = position_counts
                .get(&existing.position_session_id)
                .copied()
                .unwrap_or(0);
            let new_position_session_id = if position_count <= 1 {
                extra_close_positions.push(existing.position_session_id);
                get_or_create_position_session(conn, position_id, updated_at).await?
            } else {
                update_position_session_last_seen(conn, existing.position_session_id, updated_at)
                    .await?;
                existing.position_session_id
            };
            active_position_ids.insert(position_id.to_string());
            insert_controller_session(
                conn,
                controller,
                cid,
                updated_at,
                new_callsign_session_id,
                new_position_session_id,
            )
            .await?;
        }
    } else {
        let callsign_session_id =
            get_or_create_callsign_session(conn, prefix, suffix, updated_at).await?;
        active_callsign_ids.insert(callsign_session_id);
        let position_session_id =
            if let Some(id) = active_position_sessions.get(position_id).copied() {
                update_position_session_last_seen(conn, id, updated_at).await?;
                id
            } else {
                get_or_create_position_session(conn, position_id, updated_at).await?
            };
        active_position_ids.insert(position_id.to_string());
        insert_controller_session(
            conn,
            controller,
            cid,
            updated_at,
            callsign_session_id,
            position_session_id,
        )
        .await?;
    }

    Ok(())
}

pub async fn finalize_callsign_sessions(
    tx: &mut Transaction<'_, Postgres>,
    active_callsign_sessions: &HashSet<Uuid>,
    active_callsign_ids: &HashSet<Uuid>,
    mut extra_close_callsign: Vec<Uuid>,
    ended_at: DateTime<Utc>,
) -> Result<(), QueryError> {
    let conn = tx.as_mut();
    let mut to_close_callsign: Vec<Uuid> = active_callsign_sessions
        .difference(active_callsign_ids)
        .cloned()
        .collect();
    to_close_callsign.append(&mut extra_close_callsign);
    if !to_close_callsign.is_empty() {
        complete_callsign_sessions(conn, &to_close_callsign, ended_at).await?;
        debug!(
            closed_callsign_sessions = to_close_callsign.len(),
            "marked callsign sessions as completed"
        );
    }
    Ok(())
}

pub async fn finalize_position_sessions(
    tx: &mut Transaction<'_, Postgres>,
    active_position_sessions: &HashMap<String, Uuid>,
    active_position_ids: &HashSet<String>,
    mut extra_close_positions: Vec<Uuid>,
    ended_at: DateTime<Utc>,
) -> Result<(), QueryError> {
    let conn = tx.as_mut();
    let mut to_close_positions: Vec<Uuid> = active_position_sessions
        .iter()
        .filter_map(|(pos_id, session_id)| {
            if active_position_ids.contains(pos_id) {
                None
            } else {
                Some(*session_id)
            }
        })
        .collect();
    to_close_positions.append(&mut extra_close_positions);
    if !to_close_positions.is_empty() {
        complete_position_sessions(conn, &to_close_positions, ended_at).await?;
        debug!(
            closed_position_sessions = to_close_positions.len(),
            "marked position sessions as completed"
        );
    }
    Ok(())
}
