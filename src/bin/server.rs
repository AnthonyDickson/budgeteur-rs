use std::{
    env::{self},
    fs::OpenOptions,
    net::{Ipv4Addr, SocketAddr},
    process::exit,
    sync::Arc,
};

use axum::{
    Router,
    extract::{MatchedPath, Request},
};
use axum_server::Handle;
use clap::Parser;
use rusqlite::Connection;
use tower_http::trace::TraceLayer;
#[cfg(debug_assertions)]
use tower_livereload::LiveReloadLayer;
use tracing_subscriber::{Layer, filter, layer::SubscriberExt, util::SubscriberInitExt};

use budgeteur_rs::{AppState, build_router, graceful_shutdown, logging_middleware};

/// The REST API server for budgeteur_rs.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// File path to the application SQLite database.
    #[arg(long)]
    db_path: String,

    /// A canonical timezone in the IANA tz database, e.g. "Pacific/Auckland".
    /// If not specified, tries to auto-detect the host system's timezone.
    #[arg(short, long)]
    timezone: Option<String>,

    /// The IP address to serve from.
    #[arg(short, long, default_value = "127.0.0.1")]
    address: String,

    /// The port to serve the API from.
    #[arg(short, long, default_value_t = 3000)]
    port: u16,

    /// File path to the application logs.
    #[arg(short, long, default_value = "debug.log")]
    log_path: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    setup_logging(&args.log_path);

    let Ok(address) = args.address.parse::<Ipv4Addr>() else {
        eprintln!(
            "{} is not a valid IP address, please check it and try again.",
            args.address
        );
        exit(1);
    };

    let Ok(secret) = env::var("SECRET") else {
        eprintln!(
            "The environment variable 'SECRET' must be set.
            Please make sure to set this to a value that is difficult to guess."
        );
        exit(1);
    };

    let addr = SocketAddr::from((address, args.port));
    let conn = Connection::open(&args.db_path)
        .unwrap_or_else(|_| panic!("Could not open database file at {}: ", args.db_path));
    let Some(timezone) = get_timezone_name(args.timezone.clone()) else {
        eprint!("{} is not a valid timezone name.", args.timezone.unwrap());
        exit(1);
    };
    let app_config = match AppState::new(conn, &secret, &timezone, Default::default()) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("Could not initialize database: {error}");
            exit(1);
        }
    };

    let handle = Handle::new();
    tokio::spawn(graceful_shutdown(handle.clone()));

    let router = add_tracing_layer(build_router(app_config));

    #[cfg(debug_assertions)]
    let router = router.layer(LiveReloadLayer::new());

    tracing::info!("HTTP starting on {}", addr);
    let result = axum_server::bind(addr)
        .handle(handle)
        .serve(router.into_make_service())
        .await;
    if let Err(error) = result {
        eprintln!("Could not start server: {error}");
        exit(1);
    }
}

fn setup_logging(log_path: &str) {
    let stdout_log = tracing_subscriber::fmt::layer().pretty();

    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .expect("Could not create log file");

    let debug_log = tracing_subscriber::fmt::layer()
        .pretty()
        .with_writer(Arc::new(log_file));

    tracing_subscriber::registry()
        .with(
            stdout_log
                .with_filter(filter::LevelFilter::INFO)
                .and_then(debug_log)
                .with_filter(filter::LevelFilter::DEBUG),
        )
        .init();
}

fn add_tracing_layer(router: Router) -> Router {
    let tracing_layer = TraceLayer::new_for_http()
        .make_span_with(|req: &Request| {
            let method = req.method();
            let uri = req.uri();

            let matched_path = req
                .extensions()
                .get::<MatchedPath>()
                .map(|matched_path| matched_path.as_str());

            tracing::debug_span!("request", %method, %uri, matched_path)
        })
        // By default, `TraceLayer` will log 5xx responses but we're doing our specific
        // logging of errors so disable that
        .on_failure(());

    router
        .layer(axum::middleware::from_fn(logging_middleware))
        .layer(tracing_layer)
}

/// If some timezone name is specified, checks that it maps to a canonical timezone.
/// Otherwise, returns canonical timezone string for UTC+00.
fn get_timezone_name(timezone_arg: Option<String>) -> Option<String> {
    if let Some(timezone_name) = timezone_arg {
        match time_tz::timezones::get_by_name(&timezone_name) {
            Some(_) => Some(timezone_name),
            None => None,
        }
    } else {
        Some("Etc/UTC".to_owned())
    }
}
