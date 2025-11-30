use crate::database::queries::{
    QueryError, fetch_callsign_session_details, fetch_position_session_details,
};
use crate::helpers::ControllerAction;
use sqlx::Executor;
use sqlx::Postgres;
use std::collections::HashSet;
use tracing::{Level, debug, event_enabled, warn};
use uuid::Uuid;

pub async fn log_session_changes<'e, E>(
    executor: &mut E,
    controller_actions: &[ControllerAction],
    active_callsign_ids: &HashSet<Uuid>,
    active_position_ids: &HashSet<String>,
    closed_callsign_ids: &[Uuid],
    closed_position_ids: &[Uuid],
) -> Result<(), QueryError>
where
    for<'c> &'c mut E: Executor<'c, Database = Postgres>,
{
    if !event_enabled!(Level::DEBUG) {
        return Ok(());
    }

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
        debug!("no closed sessions");
    } else {
        debug!(controllers = ?closed_controllers, "closed controller sessions");
    }

    let closed_callsigns_details =
        fetch_callsign_session_details(&mut *executor, closed_callsign_ids)
            .await
            .unwrap_or_else(|e| {
                warn!(error = ?e, "failed to fetch closed callsign sessions details");
                Vec::default()
            });

    let closed_positions_details =
        fetch_position_session_details(&mut *executor, closed_position_ids)
            .await
            .unwrap_or_else(|e| {
                warn!(error = ?e, "failed to fetch closed position sessions details");
                Vec::default()
            });

    if closed_callsigns_details.is_empty() {
        debug!("no closed callsign sessions");
    } else {
        debug!(callsigns = ?closed_callsigns_details, "closed callsign sessions");
    }

    if closed_positions_details.is_empty() {
        debug!("no closed position sessions");
    } else {
        debug!(positions = ?closed_positions_details, "closed position sessions");
    }

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
            if active_callsign_ids.contains(callsign_session_id) {
                callsign_stayed.push(*callsign_session_id);
            }
            if active_position_ids.contains(&position_session_id.to_string()) {
                position_stayed.push(*position_session_id);
            }
        }
    }

    if !callsign_stayed.is_empty() {
        let stayed_details = fetch_callsign_session_details(&mut *executor, &callsign_stayed)
            .await
            .unwrap_or_else(|e| {
                warn!(error = ?e, "failed to fetch callsign sessions details for sessions that stayed open");
                Vec::default()
            });
        if !stayed_details.is_empty() {
            debug!(
                callsigns = ?stayed_details,
                "controller sessions closed while callsign session stayed active"
            );
        }
    }
    if !position_stayed.is_empty() {
        let stayed_positions = fetch_position_session_details(&mut *executor, &position_stayed)
            .await
            .unwrap_or_else(|e| {
                warn!(error = ?e, "failed to fetch position sessions details for sessions that stayed open");
                Vec::default()
            });
        if !stayed_positions.is_empty() {
            debug!(
                positions = ?stayed_positions,
                "controller sessions closed while position session stayed active"
            );
        }
    }

    Ok(())
}
