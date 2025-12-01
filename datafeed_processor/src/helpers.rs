use crate::database::queries::{
    QueryError, complete_callsign_sessions, complete_position_sessions,
    get_active_callsign_sessions, get_active_controller_session_keys, get_active_position_sessions,
    get_or_create_callsign_session, get_or_create_position_session,
    update_callsign_session_last_seen, update_position_session_last_seen,
};
use crate::error::{CallsignParseError, ControllerParseError};
use chrono::{DateTime, Utc};
use shared::vnas::datafeed::Controller;
use sqlx::{Postgres, Transaction};
use std::collections::{HashMap, HashSet};
use tracing::{Level, event_enabled, trace};
use uuid::Uuid;

type Callsign<'a> = (&'a str, Option<&'a str>, &'a str);

#[derive(Debug, Clone)]
pub struct ActiveControllerState {
    pub controller_session_id: Uuid,
    pub network_session_id: Uuid,
    pub login_time: DateTime<Utc>,
    pub callsign_session_id: Uuid,
    pub position_id: String,
    pub position_session_id: Uuid,
    pub connected_callsign: String,
}

pub struct ParsedController<'a> {
    pub cid: i32,
    pub prefix: &'a str,
    pub suffix: &'a str,
    pub position_id: String,
}

#[derive(Default)]
pub struct ActiveState {
    pub active_by_cid: HashMap<i32, ActiveControllerState>,
    pub active_callsign_sessions: HashSet<Uuid>,
    pub active_callsign_sessions_map: HashMap<(String, String), Uuid>,
    pub active_position_sessions: HashMap<String, Uuid>,
}

fn parse_callsign(callsign: &str) -> Result<Callsign<'_>, CallsignParseError> {
    let parts: Vec<&str> = callsign.split('_').collect();
    // Direct indexing below is safe because we have already checked the length
    match parts.len() {
        2 => Ok((parts[0], None, parts[1])),
        3 => Ok((parts[0], Some(parts[1]), parts[2])),
        other => Err(CallsignParseError::IncorrectFormat(other)),
    }
}

pub fn login_times_match(a: &DateTime<Utc>, b: &DateTime<Utc>) -> bool {
    a.timestamp_micros() == b.timestamp_micros()
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
                    controller_session_id: session.controller_session_id,
                    network_session_id: session.network_session_id,
                    login_time: session.login_time,
                    callsign_session_id: session.callsign_session_id,
                    position_id: session.primary_position_id.clone(),
                    position_session_id: session.position_session_id,
                    connected_callsign: session.connected_callsign.clone(),
                },
            )
        })
        .collect();
    state.active_callsign_sessions = get_active_callsign_sessions(&mut *conn)
        .await?
        .into_iter()
        .map(|s| {
            state
                .active_callsign_sessions_map
                .insert((s.prefix, s.suffix), s.id);
            s.id
        })
        .collect();
    state.active_position_sessions = get_active_position_sessions(&mut *conn)
        .await?
        .into_iter()
        .map(|s| (s.position_id, s.id))
        .collect();

    Ok(state)
}

pub fn parse_controller_parts(
    controller: &Controller,
) -> Result<ParsedController<'_>, ControllerParseError> {
    let cid_str = controller.vatsim_data.cid.clone();
    let cid: i32 = cid_str
        .parse()
        .map_err(|source| ControllerParseError::Cid {
            cid: cid_str,
            source,
        })?;

    let (prefix, _infix, suffix) =
        parse_callsign(&controller.vatsim_data.callsign).map_err(|source| {
            ControllerParseError::Callsign {
                callsign: controller.vatsim_data.callsign.clone(),
                source,
            }
        })?;
    Ok(ParsedController {
        cid,
        prefix,
        suffix,
        position_id: controller.primary_position_id.clone(),
    })
}

#[derive(Clone, Debug)]
pub enum ControllerAction {
    UpdateExisting {
        controller_session_id: Uuid,
        network_session_id: Uuid,
        controller: Controller,
        callsign_session_id: Uuid,
        position_session_id: Uuid,
    },
    CreateNew {
        controller: Controller,
        callsign_key: (String, String),
        position_id: String,
        cid: i32,
    },
    Close {
        controller_session_id: Uuid,
        cid: i32,
        callsign_session_id: Uuid,
        position_session_id: Uuid,
        connected_callsign: String,
        reason: ControllerCloseReason,
    },
}

#[derive(Clone, Debug)]
pub enum ControllerCloseReason {
    MissingFromDatafeed,
    ReconnectedOrChangedPosition,
    DeactivatedPosition,
}

pub async fn ensure_callsign_session(
    tx: &mut Transaction<'_, Postgres>,
    active_callsign_sessions_map: &mut HashMap<(String, String), Uuid>,
    callsign_key: &(String, String),
    seen_at: DateTime<Utc>,
) -> Result<(Uuid, bool), QueryError> {
    if let Some(id) = active_callsign_sessions_map.get(callsign_key).copied() {
        update_callsign_session_last_seen(tx.as_mut(), id, seen_at).await?;
        return Ok((id, false));
    }

    let id = get_or_create_callsign_session(tx.as_mut(), &callsign_key.0, &callsign_key.1, seen_at)
        .await?;
    active_callsign_sessions_map.insert(callsign_key.clone(), id);
    Ok((id, true))
}

pub async fn ensure_position_session(
    tx: &mut Transaction<'_, Postgres>,
    active_position_sessions: &mut HashMap<String, Uuid>,
    position_id: &str,
    seen_at: DateTime<Utc>,
) -> Result<(Uuid, bool), QueryError> {
    if let Some(id) = active_position_sessions.get(position_id).copied() {
        update_position_session_last_seen(tx.as_mut(), id, seen_at).await?;
        return Ok((id, false));
    }

    let id = get_or_create_position_session(tx.as_mut(), position_id, seen_at).await?;
    active_position_sessions.insert(position_id.to_string(), id);
    Ok((id, true))
}
//
// pub async fn finalize_controller_sessions(
//     tx: &mut Transaction<'_, Postgres>,
//     active_by_cid: &HashMap<i32, ActiveControllerState>,
//     controllers_to_complete: &mut Vec<Uuid>,
//     ended_at: DateTime<Utc>,
// ) -> Result<u64, QueryError> {
//     controllers_to_complete.extend(
//         active_by_cid
//             .values()
//             .map(|state| state.controller_session_id),
//     );
//
//     if controllers_to_complete.is_empty() {
//         return Ok(0);
//     }
//
//     complete_controller_sessions(tx.as_mut(), controllers_to_complete, ended_at).await
// }

pub async fn finalize_callsign_sessions(
    tx: &mut Transaction<'_, Postgres>,
    active_callsign_sessions: &HashSet<Uuid>,
    active_callsign_ids: &HashSet<Uuid>,
    ended_at: DateTime<Utc>,
) -> Result<Vec<Uuid>, QueryError> {
    let conn = tx.as_mut();
    let to_close_callsign: Vec<Uuid> = active_callsign_sessions
        .difference(active_callsign_ids)
        .cloned()
        .collect();

    if event_enabled!(Level::TRACE) {
        for callsign_id in &to_close_callsign {
            trace!(id = %callsign_id, "closing callsign session");
        }
    }

    if !to_close_callsign.is_empty() {
        complete_callsign_sessions(conn, &to_close_callsign, ended_at).await?;
    }

    Ok(to_close_callsign)
}

pub async fn finalize_position_sessions(
    tx: &mut Transaction<'_, Postgres>,
    active_position_sessions: &HashMap<String, Uuid>,
    active_position_ids: &HashSet<String>,
    ended_at: DateTime<Utc>,
) -> Result<Vec<Uuid>, QueryError> {
    let conn = tx.as_mut();
    let to_close_positions: Vec<Uuid> = active_position_sessions
        .iter()
        .filter_map(|(pos_id, session_id)| {
            if active_position_ids.contains(pos_id) {
                None
            } else {
                Some(*session_id)
            }
        })
        .collect();

    if event_enabled!(Level::TRACE) {
        for position_session_id in &to_close_positions {
            trace!(id = %position_session_id, "closing position session");
        }
    }

    if !to_close_positions.is_empty() {
        complete_position_sessions(conn, &to_close_positions, ended_at).await?;
    }

    Ok(to_close_positions)
}
