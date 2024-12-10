use crate::{injector::types::response, state};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{on, MethodFilter},
    Json,
    Router,
};

pub fn router() -> Router<state::State> {
    Router::new().route("/", on(MethodFilter::GET, root))
}

async fn root(
    State(state): State<state::State>,
    Path(server_id): Path<String>,
) -> impl IntoResponse {
    if state.servers.get(&server_id).is_none() {
        return StatusCode::NO_CONTENT.into_response();
    }

    let response = response::root::Root {
        meta: response::root::meta::Meta {
            server_name: Some(server_id),
            implementation_name: Some("Auth-Proxy-GL".to_string()),
            implementation_version: None,
        },
        skin_domains: Vec::new(),
        signature_public_key: state.key_pair.public.to_string(),
    };

    (StatusCode::OK, Json(response)).into_response()
}
