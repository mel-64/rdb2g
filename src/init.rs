use anyhow::Result;
use clap::Parser;
use log::debug;
use reqwest::Client;
use reqwest::header;
use std::str::{self, FromStr};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, SystemTime};
use tokio::net;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None, arg_required_else_help=true)]
pub(crate) struct Args {
    #[arg(
        long,
        default_value = "tcp://0.0.0.0:8080",
        env,
        help = "Address to bind to, starting with either tcp:// or unix://"
    )]
    pub(crate) bind_addr: BindAddr,

    #[arg(short, long, default_value = "https://img.shields.io", env)]
    pub(crate) shield_io_instance: String,

    #[arg(
        short = 'r',
        long,
        default_value_t = 60,
        help = "Time in seconds until cache entry goes stale and has to be refetched",
        env
    )]
    pub(crate) refresh_timeout: u64,

    #[arg(short, long, default_value = "Outdated_Ebuilds", env)]
    pub(crate) badge_text: String,

    #[arg(long, default_value = "green", env)]
    pub(crate) badge_color_zero: String,

    #[arg(long, default_value = "red", env)]
    pub(crate) badge_color_one_or_more: String,

    #[arg(long, default_value = "/", env)]
    pub(crate) subpath: String,

    #[arg(
        help = "Forgejo issue api URL to scrape (/api/v1/repos/{owner}/{repo}/issue/comments/{id})",
        index = 1,
        env
    )]
    pub(crate) issue: String,
}

#[derive(Debug, Clone)]
pub(crate) enum BindAddr {
    SocketAddrUnix(net::unix::SocketAddr),
    SocketAddrIp(std::net::SocketAddr),
}

impl FromStr for BindAddr {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> std::prelude::v1::Result<Self, Self::Err> {
        get_bind_addr(s)
    }
}

#[derive(Debug)]
pub(crate) struct State {
    pub(crate) updates_available: Arc<Mutex<usize>>,
    pub(crate) last_update: Arc<Mutex<SystemTime>>,
    pub(crate) client: Client,
    pub(crate) conf: Args,
}

impl State {
    pub(crate) fn get_time_passed(&self) -> Duration {
        let last_update = *Arc::clone(&self.last_update).lock().unwrap();
        SystemTime::now().duration_since(last_update).unwrap()
    }
}

pub(crate) static STATE: LazyLock<State> = LazyLock::new(|| {
    debug!("Constructing state");
    State {
        updates_available: Arc::new(Mutex::new(0)),
        last_update: Arc::new(Mutex::new(SystemTime::UNIX_EPOCH)),
        client: construct_client(),
        conf: Args::parse(), // Is only read therefore no Mutex required
    }
});

fn construct_client() -> Client {
    let useragent = format!("Renovate Dep Board Badge Gen: {}", clap::crate_version!());
    debug!("Client useragent: {}", useragent);

    let mut headers = header::HeaderMap::new();
    headers.insert(header::ACCEPT, "application/json".parse().unwrap());

    reqwest::ClientBuilder::new()
        .user_agent(useragent)
        .read_timeout(Duration::from_secs(10))
        .http2_keep_alive_interval(Duration::from_secs(25))
        .http2_keep_alive_while_idle(true)
        .build()
        .unwrap()
}

fn get_bind_addr(addr: &str) -> Result<BindAddr> {
    let ip_err = anyhow::Error::msg(format!("Not a valid bind address: {}", addr));
    if let Some(res) = addr.strip_prefix("tcp://") {
        return Ok(BindAddr::SocketAddrIp(res.parse()?));
    } else if let Some(res) = addr.strip_prefix("unix://") {
        return Ok(BindAddr::SocketAddrUnix(
            std::os::unix::net::SocketAddr::from_pathname(res)?.into(),
        ));
    }
    Err(ip_err)
}
