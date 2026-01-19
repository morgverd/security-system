use crate::alerts::{send_alert, AlertInfo, AlertLevel};
use log::{error, info};
use warp::Filter;

#[derive(Debug)]
struct AuthError;
impl warp::reject::Reject for AuthError {}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AlarmEvent {
    input1: Option<String>,
    extra_text: String,
}

async fn handle_cctv_webhook(
    _: (),
    payload: AlarmEvent,
) -> Result<impl warp::Reply, warp::Rejection> {
    info!("Received CCTV webhook: {payload:?}");

    let alert = AlertInfo {
        source: "cctv-webhook".to_string(),
        message: payload.extra_text,
        level: if payload.input1 == Some("test".to_string()) {
            AlertLevel::Alarm
        } else {
            AlertLevel::Critical
        },
        timestamp: None,
    };
    let _ = send_alert(alert).await;

    Ok(warp::reply::json(&serde_json::json!({
        "status": "success",
        "message": "CCTV webhook processed"
    })))
}

async fn handle_rejection(
    err: warp::Rejection,
) -> Result<impl warp::Reply, std::convert::Infallible> {
    let (code, message) = if err.is_not_found() {
        (warp::http::StatusCode::NOT_FOUND, "Not Found")
    } else if err.find::<AuthError>().is_some() {
        (
            warp::http::StatusCode::UNAUTHORIZED,
            "Invalid Authorization header",
        )
    } else if err.find::<warp::reject::MissingHeader>().is_some() {
        (
            warp::http::StatusCode::BAD_REQUEST,
            "Missing required header",
        )
    } else if err.find::<warp::reject::InvalidQuery>().is_some() {
        (
            warp::http::StatusCode::BAD_REQUEST,
            "Invalid query parameters",
        )
    } else if err.find::<warp::reject::MethodNotAllowed>().is_some() {
        (
            warp::http::StatusCode::METHOD_NOT_ALLOWED,
            "Method not allowed",
        )
    } else if err.find::<warp::reject::PayloadTooLarge>().is_some() {
        (
            warp::http::StatusCode::PAYLOAD_TOO_LARGE,
            "Payload too large",
        )
    } else if err.find::<warp::reject::UnsupportedMediaType>().is_some() {
        (
            warp::http::StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "Unsupported media type",
        )
    } else if err.find::<warp::reject::LengthRequired>().is_some() {
        (
            warp::http::StatusCode::LENGTH_REQUIRED,
            "Content-Length header is required",
        )
    } else if err.find::<warp::body::BodyDeserializeError>().is_some() {
        (warp::http::StatusCode::BAD_REQUEST, "Invalid request body")
    } else {
        error!("Unhandled rejection: {err:?}");
        (
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error",
        )
    };

    let json_reply = warp::reply::json(&serde_json::json!({
        "success": false,
        "error_message": message
    }));
    Ok(warp::reply::with_status(json_reply, code))
}

pub(crate) fn get_routes(
) -> impl Filter<Extract = (impl warp::Reply,), Error = std::convert::Infallible> + Clone {
    let auth_header = warp::header::<String>("Authorization").and_then(|v: String| async move {
        if v == "hello" {
            Ok(())
        } else {
            Err(warp::reject::custom(AuthError))
        }
    });

    warp::post()
        .and(warp::path("cctv"))
        .and(auth_header)
        .and(warp::body::json())
        .and_then(handle_cctv_webhook)
        .recover(handle_rejection)
}
