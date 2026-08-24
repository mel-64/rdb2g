use clap::Parser;
use log::debug;
use reqwest::Client;
use reqwest::header;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, SystemTime};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None, arg_required_else_help=true)]
pub(crate) struct Args {
    #[arg(long = "host", default_value = "0.0.0.0")]
    pub(crate) bind_host: String,

    #[arg(short = 'p', long = "port", default_value_t = 8080)]
    pub(crate) bind_port: u16,

    #[arg(short, long, default_value = "https://img.shields.io")]
    pub(crate) shield_io_instance: String,

    #[arg(
        short = 'r',
        long,
        default_value_t = 60,
        help = "Time in seconds until cache entry goes stale and has to be refetched"
    )]
    pub(crate) refresh_timeout: u64,

    #[arg(short, long, default_value = "Outdated_Ebuilds")]
    pub(crate) badge_text: String,

    #[arg(long, default_value = "green")]
    pub(crate) badge_color_zero: String,

    #[arg(long, default_value = "red")]
    pub(crate) badge_color_one_or_more: String,

    #[arg(
        help = "Forgejo issue api URL to scrape (/api/v1/repos/{owner}/{repo}/issue/comments/{id})",
        index = 1
    )]
    pub(crate) issue: String,
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
