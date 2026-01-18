use std::convert::Infallible;
use log::{info, error};
use warp::{Filter, Rejection, Reply};
use serde::Deserialize;
use warp::body::BodyDeserializeError;
use warp::http::StatusCode;
use warp::reject::{InvalidQuery, LengthRequired, MethodNotAllowed, MissingHeader, PayloadTooLarge, UnsupportedMediaType};
use crate::alerts::{AlertInfo, AlertLevel, send_alert};

#[derive(Debug)]
struct AuthError;
impl warp::reject::Reject for AuthError {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AlarmEvent {
    input1: Option<String>,
    extra_text: String
}

async fn handle_cctv_webhook(_: (), payload: AlarmEvent) -> Result<impl Reply, Rejection> {
    info!("Received CCTV webhook: {:?}", payload);

    let alert = AlertInfo {
        source: "CCTV".to_string(),
        message: payload.extra_text,
        level: if payload.input1 == Some("test".to_string()) { AlertLevel::Alarm } else { AlertLevel::Critical },
        timestamp: None
    };
    let _ = send_alert(alert).await;

    Ok(warp::reply::json(&serde_json::json!({
        "status": "success",
        "message": "CCTV webhook processed"
    })))
}

async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let (code, message) = if err.is_not_found() {
        (StatusCode::NOT_FOUND, "Not Found")
    } else if err.find::<AuthError>().is_some() {
        (StatusCode::UNAUTHORIZED, "Invalid Authorization header")
    } else if err.find::<MissingHeader>().is_some() {
        (StatusCode::BAD_REQUEST, "Missing required header")
    } else if err.find::<InvalidQuery>().is_some() {
        (StatusCode::BAD_REQUEST, "Invalid query parameters")
    } else if err.find::<MethodNotAllowed>().is_some() {
        (StatusCode::METHOD_NOT_ALLOWED, "Method not allowed")
    } else if err.find::<PayloadTooLarge>().is_some() {
        (StatusCode::PAYLOAD_TOO_LARGE, "Payload too large")
    } else if err.find::<UnsupportedMediaType>().is_some() {
        (StatusCode::UNSUPPORTED_MEDIA_TYPE, "Unsupported media type")
    } else if err.find::<LengthRequired>().is_some() {
        (StatusCode::LENGTH_REQUIRED, "Content-Length header is required")
    } else if err.find::<BodyDeserializeError>().is_some() {
        (StatusCode::BAD_REQUEST, "Invalid request body")
    } else {
        error!("Unhandled rejection: {:?}", err);
        (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")
    };

    let json_reply = warp::reply::json(&serde_json::json!({
        "success": false,
        "error_message": message
    }));
    Ok(warp::reply::with_status(json_reply, code))
}

pub(crate) fn get_routes() -> impl Filter<Extract = (impl Reply,), Error = Infallible> + Clone {
    let auth_header = warp::header::<String>("Authorization")
        .and_then(|v: String| async move {
            if v == "hello" {
                Ok(())
            } else {
                Err(warp::reject::custom(AuthError))
            }
        });

    warp::post()
        .and(warp::path("cctv"))
        .and(auth_header.clone())
        .and(warp::body::json())
        .and_then(handle_cctv_webhook)
        .recover(handle_rejection)
}