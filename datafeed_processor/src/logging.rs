use crate::database::queries::{
    QueryError, fetch_callsign_session_details, fetch_position_session_details,
};
use crate::helpers::ControllerAction;
use sqlx::{Pool, Postgres, Transaction};
use std::collections::HashSet;
use tracing::{Level, debug, event_enabled, warn};
use uuid::Uuid;

pub async fn debug_log_sessions_changes(
    pool: &Pool<Postgres>,
    controller_actions: &[ControllerAction],
    new_callsign_session_ids: &HashSet<Uuid>,
    new_position_session_ids: &HashSet<Uuid>,
    closed_callsign_session_ids: &[Uuid],
    closed_position_session_ids: &[Uuid],
) -> Result<(), QueryError> {
    if !event_enabled!(Level::DEBUG) {
        return Ok(());
    }

    let mut tx = pool.begin().await?;
    let closed_callsign_set: HashSet<Uuid> = closed_callsign_session_ids.iter().copied().collect();
    let closed_position_set: HashSet<Uuid> = closed_position_session_ids.iter().copied().collect();

    let created_controllers = controller_actions
        .iter()
        .filter_map(|a| {
            if let ControllerAction::CreateNew {
                controller, cid, ..
            } = a
            {
                Some((cid, controller.vatsim_data.callsign.clone()))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let closed_controllers = controller_actions
        .iter()
        .filter_map(|a| {
            if let ControllerAction::Close {
                cid,
                connected_callsign,
                reason,
                ..
            } = a
            {
                Some((cid, connected_callsign, reason))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if created_controllers.is_empty() {
        debug!("no new controller sessions");
    } else {
        debug!(controllers = ?created_controllers, "created new controller sessions");
    }

    if closed_controllers.is_empty() {
        debug!("no closed controller sessions");
    } else {
        debug!(controllers = ?closed_controllers, "closed controller sessions");
    }

    log_callsign_sessions(
        &mut tx,
        new_callsign_session_ids,
        "no opened callsign sessions",
        "opened callsign sessions",
    )
    .await?;

    log_position_sessions(
        &mut tx,
        new_position_session_ids,
        "no opened position sessions",
        "opened position sessions",
    )
    .await?;

    log_callsign_sessions(
        &mut tx,
        &closed_callsign_set,
        "no closed callsign sessions",
        "closed callsign sessions",
    )
    .await?;

    log_position_sessions(
        &mut tx,
        &closed_position_set,
        "no closed position sessions",
        "closed position sessions",
    )
    .await?;

    // Log sessions that stayed active while a controller closed.
    let mut callsign_stayed: Vec<_> = Vec::new();
    let mut position_stayed: Vec<_> = Vec::new();
    for action in controller_actions {
        if let ControllerAction::Close {
            callsign_session_id,
            position_session_id,
            ..
        } = action
        {
            if !closed_callsign_set.contains(callsign_session_id) {
                callsign_stayed.push(*callsign_session_id);
            }
            // A position session stayed active if it was not part of the closed set.
            if !closed_position_set.contains(position_session_id) {
                position_stayed.push(*position_session_id);
            }
        }
    }

    log_callsign_sessions(
        &mut tx,
        &callsign_stayed.iter().copied().collect(),
        "",
        "callsign sessions remain active although controller sessions closed",
    )
    .await?;

    log_position_sessions(
        &mut tx,
        &position_stayed.iter().copied().collect(),
        "",
        "position sessions remain active although controller sessions closed",
    )
    .await?;

    tx.commit().await?;

    Ok(())
}

async fn log_callsign_sessions(
    tx: &mut Transaction<'_, Postgres>,
    ids: &HashSet<Uuid>,
    empty_message: &str,
    log_message: &str,
) -> Result<(), QueryError> {
    if ids.is_empty() && !empty_message.is_empty() {
        debug!("{}", empty_message);
        return Ok(());
    }

    let details =
        fetch_callsign_session_details(&mut **tx, &ids.iter().copied().collect::<Vec<_>>())
            .await
            .unwrap_or_else(|e| {
                warn!(error = ?e, "failed to fetch callsign sessions details");
                Vec::default()
            });

    if details.is_empty() && !empty_message.is_empty() {
        debug!("{}", empty_message);
    } else {
        debug!(callsigns = ?details, "{}", log_message);
    }

    Ok(())
}

async fn log_position_sessions(
    tx: &mut Transaction<'_, Postgres>,
    ids: &HashSet<Uuid>,
    empty_message: &str,
    log_message: &str,
) -> Result<(), QueryError> {
    if ids.is_empty() && !empty_message.is_empty() {
        debug!("{}", empty_message);
        return Ok(());
    }

    let details =
        fetch_position_session_details(&mut **tx, &ids.iter().copied().collect::<Vec<_>>())
            .await
            .unwrap_or_else(|e| {
                warn!(error = ?e, "failed to fetch position sessions details");
                Vec::default()
            });

    if details.is_empty() && !empty_message.is_empty() {
        debug!("{}", empty_message);
    } else {
        debug!(positions = ?details, "{}", log_message);
    }

    Ok(())
}
