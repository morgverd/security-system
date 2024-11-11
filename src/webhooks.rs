use std::convert::Infallible;
use log::error;
use warp::{Filter, Rejection, Reply};
use serde::Deserialize;

#[derive(Debug)]
struct AuthError;
impl warp::reject::Reject for AuthError {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AlarmEvent {
    input1: Option<String>,
    event_type: String,
    extra_text: String,
    date_time: String
}

async fn handle_cctv_webhook(_: (), payload: AlarmEvent) -> Result<impl Reply, Rejection> {
    println!("Received CCTV webhook: {:?}", payload);
    Ok(warp::reply::json(&serde_json::json!({
        "status": "success",
        "message": "CCTV webhook processed"
    })))
}

async fn handle_alarm_webhook(_: ()) -> Result<impl Reply, Rejection> {
    println!("Received alarm webhook");
    Ok(warp::reply::json(&serde_json::json!({
        "status": "success",
        "message": "Alarm webhook processed"
    })))
}

async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let (code, message) = if err.is_not_found() {
        (warp::http::StatusCode::NOT_FOUND, "Not Found")
    } else if err.find::<AuthError>().is_some() {
        (warp::http::StatusCode::UNAUTHORIZED, "Invalid Authorization header")
    } else if err.find::<warp::reject::MissingHeader>().is_some() {
        (warp::http::StatusCode::BAD_REQUEST, "Missing Authorization header")
    } else {
        error!("unhandled error: {:?}", err);
        (warp::http::StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")
    };

    Ok(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({
            "success": false,
            "error_message": message
        })),
        code
    ))
}

pub(crate) fn get_routes() -> impl Filter<Extract = (impl Reply,), Error = Infallible> + Clone {
    let auth_header = warp::header::<String>("Authorization")
        .and_then(|v: String| async move {
            println!("{v}");
            if v == "hello" {
                Ok(())
            } else {
                Err(warp::reject::custom(AuthError))
            }
        });

    let cctv_webhook = warp::post()
        .and(warp::path("cctv"))
        .and(auth_header.clone())
        .and(warp::body::json())
        .and_then(handle_cctv_webhook);

    let alarm_webhook = warp::post()
        .and(warp::path("alarm"))
        .and(auth_header)
        .and_then(handle_alarm_webhook);

    cctv_webhook
        .or(alarm_webhook)
        .recover(handle_rejection)
}