use crate::injector::types::request;
use crate::injector::types::response::profile;
use crate::injector::types::response::profile::property::textures;
use crate::injector::types::response::profile::property::textures::kind::skin::metadata;
use crate::injector::types::response::profile::property::textures::kind::{cape, skin};
use crate::state;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{on, MethodFilter};
use axum::Json;
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub fn router() -> axum::Router<state::State> {
    axum::Router::new()
        .route("/hasJoined", on(MethodFilter::GET, has_joined))
        .route("/profile/:uuid", on(MethodFilter::GET, profile_by_uuid))
}

async fn has_joined(
    State(state): State<state::State>,
    Path(server_id): Path<String>,
    Query(query): Query<request::has_joined::Query>,
) -> impl IntoResponse {
    let Some(socket) = state.sockets.socket(server_id) else {
        return StatusCode::NO_CONTENT.into_response();
    };

    let Ok(check_server) = ({
        let socket = socket.lock().await;

        socket
            .check_server(query.username, query.server_id, false, false)
            .await
    }) else {
        return StatusCode::NO_CONTENT.into_response();
    };

    let response = profile::Profile {
        id: check_server.uuid.simple().to_string(),
        name: check_server.profile.username,
        properties: Vec::new(),
    };

    (StatusCode::OK, Json(response)).into_response()
}

async fn profile_by_uuid(
    State(state): State<state::State>,
    Path((server_id, uuid)): Path<(String, Uuid)>,
    Query(query): Query<request::profile_by_uuid::Query>,
) -> impl IntoResponse {
    let Some(socket) = state.sockets.socket(server_id) else {
        return StatusCode::NO_CONTENT.into_response();
    };

    let Ok(profile) = ({
        let socket = socket.lock().await;

        socket.get_profile_by_uuid(uuid).await
    }) else {
        return StatusCode::NO_CONTENT.into_response();
    };

    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

    let skin = profile.player_profile.assets.skin.map(|skin| skin::Skin {
        url: skin.url,
        metadata: skin.metadata.map(|_| metadata::Metadata {
            model: metadata::Model::Slim,
        }),
    });
    let cape = profile
        .player_profile
        .assets
        .cape
        .map(|cape| cape::Cape { url: cape.url });

    let textures = textures::Textures {
        timestamp: now.as_millis(),
        profile_id: profile.player_profile.uuid,
        profile_name: profile.player_profile.username.clone(),
        signature_required: !query.unsigned,
        textures: textures::kind::Kind { skin, cape },
    };

    let encoded = BASE64_STANDARD.encode(serde_json::to_string(&textures).unwrap());

    let response = profile::Profile {
        id: profile.player_profile.uuid.simple().to_string(),
        name: profile.player_profile.username,
        properties: vec![profile::property::Property {
            name: "textures".to_string(),
            value: encoded,
            signature: None,
        }],
    };

    (StatusCode::OK, Json(response)).into_response()
}
