use crate::injector::types::request::profiles_by_usernames;
use crate::injector::types::response::profile;
use crate::state;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{on, MethodFilter};
use axum::Json;
use futures_util::{future, StreamExt};

pub fn router() -> axum::Router<state::State> {
    axum::Router::new().route(
        "/profiles/minecraft",
        on(MethodFilter::POST, profiles_by_usernames),
    )
}

async fn profiles_by_usernames(
    State(state): State<state::State>,
    Path(server_id): Path<String>,
    Json(profiles_by_usernames::Body(usernames)): Json<profiles_by_usernames::Body>,
) -> impl IntoResponse {
    let Some(socket) = state.sockets.socket(server_id) else {
        return StatusCode::NO_CONTENT.into_response();
    };

    let prepared = futures::stream::iter(usernames)
        .map(|username| {
            let socket = socket.clone();

            async move {
                let socket = socket.lock().await;

                socket.get_profile_by_username(username).await
            }
        })
        .collect::<Vec<_>>()
        .await;

    let response = future::join_all(prepared)
        .await
        .into_iter()
        .filter_map(|maybe_profile| maybe_profile.ok())
        .map(|profile| profile::Profile {
            id: profile.player_profile.uuid.simple().to_string(),
            name: profile.player_profile.username,
            properties: Vec::new(),
        })
        .collect::<Vec<_>>();

    (StatusCode::OK, Json(response)).into_response()
}
