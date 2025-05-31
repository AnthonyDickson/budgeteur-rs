use std::{
    env::{self},
    fs::OpenOptions,
    net::SocketAddr,
    sync::{Arc, Mutex},
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

use budgeteur_rs::{
    AppState, build_router, graceful_shutdown, logging_middleware,
    stores::sqlite::{
        SQLiteBalanceStore, SQLiteCategoryStore, SQLiteTransactionStore, SQLiteUserStore,
    },
};

/// The REST API server for budgeteur_rs.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// File path to the application SQLite database.
    #[arg(long)]
    db_path: String,

    /// The port to serve the API from.
    #[arg(short, long, default_value_t = 3000)]
    port: u16,
}

#[tokio::main]
async fn main() {
    setup_logging();

    let args = Args::parse();
    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));
    let conn = Connection::open(&args.db_path).unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let secret = env::var("SECRET").expect("The environment variable 'SECRET' must be set");
    let app_config = AppState::new(
        &secret,
        SQLiteBalanceStore::new(conn.clone()),
        SQLiteCategoryStore::new(conn.clone()),
        SQLiteTransactionStore::new(conn.clone()),
        SQLiteUserStore::new(conn.clone()),
    );

    let handle = Handle::new();
    tokio::spawn(graceful_shutdown(handle.clone()));

    let router = add_tracing_layer(build_router(app_config));

    #[cfg(debug_assertions)]
    let router = router.layer(LiveReloadLayer::new());

    tracing::info!("HTTP server listening on {}", addr);
    axum_server::bind(addr)
        .handle(handle)
        .serve(router.into_make_service())
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
        // By default, `TraceLayer` will log 5xx responses but we're doing our specific
        // logging of errors so disable that
        .on_failure(());

    router
        .layer(axum::middleware::from_fn(logging_middleware))
        .layer(tracing_layer)
}
