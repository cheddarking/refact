use std::sync::Arc;
use axum::Extension;
use axum::http::{Response, StatusCode};
use hyper::Body;
use crate::at_tools::at_tools_dict::at_tools_dicts;
use crate::custom_error::ScratchError;
use crate::global_context::GlobalContext;
use tokio::sync::RwLock as ARwLock;


pub async fn handle_v1_tools_available(
    Extension(_global_context): Extension<Arc<ARwLock<GlobalContext>>>,
    _: hyper::body::Bytes,
)  -> axum::response::Result<Response<Body>, ScratchError> {
    let at_dict = at_tools_dicts().map_err(|e| {
        tracing::warn!("can't load at_commands_dicts: {}", e);
        return ScratchError::new(StatusCode::NOT_FOUND, format!("can't load at_commands_dicts: {}", e));
    })?;
    let body = serde_json::to_string_pretty(
        &at_dict.iter().map(|x|x.clone().into_openai_style()).collect::<Vec<_>>()
    ).map_err(|e| ScratchError::new(StatusCode::UNPROCESSABLE_ENTITY, format!("JSON problem: {}", e)))?;
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Body::from(body))
        .unwrap()
    )
}
