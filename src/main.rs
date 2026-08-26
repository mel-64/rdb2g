use anyhow::Result;
use axum::response::{IntoResponse, Response};
use axum::{Router, response::Redirect, routing::get};
use log::{debug, error};
use reqwest::StatusCode;
use serde::Deserialize;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, UnixListener};
use tokio::{io, signal};

use crate::init::{BindAddr, STATE};

mod init;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    debug!("Current state: {:?}", init::STATE);
    let conf = &init::STATE.conf;
    let mut out = io::stdout();

    tokio::spawn(refresh_state(
        get_badge_number()
            .await
            .expect("First badge counting fetch failed! Exiting.."),
    ));

    let _ = out.write_all(b"Config valid, starting server..\n").await;
    let _ = out.flush().await;

    let app = Router::new().route(&conf.subpath, get(handle));
    match conf.bind_addr.clone() {
        BindAddr::SocketAddrIp(res) => {
            let _serve = axum::serve(TcpListener::bind(res).await?, app)
                .with_graceful_shutdown(shutdown_signal())
                .await?;
        }
        BindAddr::SocketAddrUnix(res) => {
            let _serve = axum::serve(UnixListener::bind_addr(&res)?, app)
                .with_graceful_shutdown(shutdown_signal())
                .await?;
        }
    };
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

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    #[cfg(unix)]
    if let BindAddr::SocketAddrUnix(res) = &STATE.conf.bind_addr {
        let _res = std::fs::remove_file(res.as_pathname().unwrap()).unwrap();
    }
}
