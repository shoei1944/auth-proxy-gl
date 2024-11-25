use crate::injector::types::request::profiles_by_usernames;
use crate::injector::types::response::profile;
use crate::{launcher, state};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{on, MethodFilter};
use axum::{Json, Router};

pub fn router() -> Router<state::State> {
    Router::new().route(
        "/profiles/minecraft",
        on(MethodFilter::POST, profiles_by_usernames),
    )
}

async fn profiles_by_usernames(
    State(state): State<state::State>,
    Path(server_id): Path<String>,
    Json(profiles_by_usernames::Body(usernames)): Json<profiles_by_usernames::Body>,
) -> impl IntoResponse {
    let Some(current_server) = state.config.servers.get(&server_id) else {
        return StatusCode::NO_CONTENT.into_response();
    };
    let Some(socket) = state.sockets.socket(server_id) else {
        return StatusCode::NO_CONTENT.into_response();
    };

    let Ok(profiles) =
        launcher::socket::execute_with_token_restore(socket.clone(), current_server, || {
            socket.batch_profiles_by_usernames(usernames.clone())
        })
        .await
    else {
        return StatusCode::NO_CONTENT.into_response();
    };

    let response = profiles
        .player_profiles
        .into_iter()
        .map(|profile| profile::Profile {
            id: profile.uuid.simple().to_string(),
            name: profile.username,
            properties: Vec::new(),
        })
        .collect::<Vec<_>>();

    (StatusCode::OK, Json(response)).into_response()
}
