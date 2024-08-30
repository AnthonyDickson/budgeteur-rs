use std::{
    env::{self},
    fs::OpenOptions,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
};

use axum::{
    extract::{MatchedPath, Request},
    Router,
};
use axum_server::{tls_rustls::RustlsConfig, Handle};
use clap::Parser;
use rusqlite::Connection;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{filter, layer::SubscriberExt, util::SubscriberInitExt, Layer};

use backrooms_rs::{build_router, graceful_shutdown, AppConfig};

/// The REST API server for backrooms_rs.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// File path to the application SQLite database.
    #[arg(long)]
    db_path: String,

    /// File path to an SSL certificate `cert.pem` and key `key.pem`.
    #[arg(long)]
    cert_path: String,

    /// The port to serve the API from.
    #[arg(short, long, default_value_t = 3000)]
    port: u16,
}

#[tokio::main]
async fn main() {
    setup_logging();

    let args = Args::parse();

    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));

    let tls_config = RustlsConfig::from_pem_file(
        PathBuf::from(&args.cert_path).join("cert.pem"),
        PathBuf::from(&args.cert_path).join("key.pem"),
    )
    .await
    .expect("Could not open TLS certificates.");

    let jwt_secret =
        env::var("JWT_SECRET").expect("The environment variable 'JWT_SECRET' must be set");

    let conn = Connection::open(&args.db_path).unwrap();
    let app_config = AppConfig::new(conn, jwt_secret);

    let handle = Handle::new();
    tokio::spawn(graceful_shutdown(handle.clone()));

    tracing::info!("HTTPS server listening on {}", addr);
    axum_server::bind_rustls(addr, tls_config)
        .handle(handle)
        .serve(add_tracing_layer(build_router().with_state(app_config)).into_make_service())
        .await
        .unwrap();
}

fn setup_logging() {
    let stdout_log = tracing_subscriber::fmt::layer().pretty();

    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("debug.log")
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
        // By default `TraceLayer` will log 5xx responses but we're doing our specific
        // logging of errors so disable that
        .on_failure(());

    router.layer(tracing_layer)
}
