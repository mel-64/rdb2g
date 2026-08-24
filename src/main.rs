use anyhow::{Context, Result};
use axum::response::{IntoResponse, Response};
use axum::{Router, response::Redirect, routing::get};
use log::{debug, error, info};
use reqwest::StatusCode;
use serde::Deserialize;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

mod init;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    debug!("Current state: {:?}", init::STATE);
    let conf = &init::STATE.conf;
    let bind_string = format!("{}:{}", conf.bind_host, conf.bind_port);
    tokio::spawn(refresh_state(
        get_badge_number()
            .await
            .expect("First badge counting fetch failed! Exiting.."),
    ));
    let app = Router::new().route("/", get(handle));
    let listener = tokio::net::TcpListener::bind(&bind_string)
        .await
        .with_context(|| format!("Couldn't bind to {}", bind_string))?;
    info!("Starting webserver on {}", bind_string);
    axum::serve(listener, app)
        .await
        .with_context(|| format!("Couldn't start webserver on {}", bind_string))?;
    Ok(())
}

async fn get_badge_number() -> Result<usize> {
    let client = &init::STATE.client;

    #[derive(Deserialize)]
    struct ResponseStruct {
        body: String, // The only field we care about- Other fields are ignored.
    }

    debug!("New outgoing counting request");

    let res = client
        .get(&init::STATE.conf.issue)
        .send()
        .await?
        .error_for_status()?
        .json::<ResponseStruct>()
        .await?;

    let checkbox_count = res.body.matches("[ ]").count();
    debug!("New outgoing counting request result: {}", checkbox_count);
    Ok(checkbox_count)
}

async fn refresh_state(count: usize) {
    let updates_available = Arc::clone(&init::STATE.updates_available);
    let mut updates_available = updates_available.lock().unwrap();
    *updates_available = count;

    let last_update = Arc::clone(&init::STATE.last_update);
    let mut last_update = last_update.lock().unwrap();
    *last_update = SystemTime::now();
    debug!("Refreshed badge count");
}

async fn handle() -> Response {
    debug!("Received new request");

    let count =
        if init::STATE.get_time_passed() >= Duration::from_secs(init::STATE.conf.refresh_timeout) {
            debug!("Getting new badge info");

            let badge_number_res = get_badge_number().await;
            let Ok(count_res) = badge_number_res else {
                let error = format!("Backend error {:?}", badge_number_res.err().unwrap());
                error!("Failed to refresh badge info: {error}");
                return (StatusCode::INTERNAL_SERVER_ERROR, error).into_response();
            };

            tokio::spawn(refresh_state(count_res));
            count_res
        } else {
            debug!("Using cached badge info");
            *Arc::clone(&init::STATE.updates_available).lock().unwrap()
        };

    let color = if count == 0 {
        &init::STATE.conf.badge_color_zero
    } else {
        &init::STATE.conf.badge_color_one_or_more
    };

    Redirect::temporary(&format!(
        "{}/badge/{}-{}-{}",
        init::STATE.conf.shield_io_instance,
        init::STATE.conf.badge_text,
        count,
        color
    ))
    .into_response()
}
