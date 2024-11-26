use crate::injector::types::response;
use crate::state;
use axum::extract::State;
use axum::routing::{on, MethodFilter};
use axum::{Json, Router};

pub fn router() -> Router<state::State> {
    Router::new().route("/", on(MethodFilter::GET, root))
}

async fn root(State(state): State<state::State>) -> Json<response::root::Root> {
    let response = response::root::Root {
        meta: response::root::meta::Meta {
            server_name: None,
            implementation_name: None,
            implementation_version: None,
        },
        skin_domains: Vec::new(),
        signature_public_key: state.key_pair.public.to_string(),
    };

    Json(response)
}
