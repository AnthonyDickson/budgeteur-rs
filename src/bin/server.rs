use std::{env, env::args, net::SocketAddr, path::PathBuf, process::exit};

use axum::extract::{MatchedPath, Request};
use axum_server::{tls_rustls::RustlsConfig, Handle};
use rusqlite::Connection;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{filter, layer::SubscriberExt, util::SubscriberInitExt, Layer};

use backrooms_rs::{build_router, graceful_shutdown, parse_port_or_default, AppConfig};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .pretty()
                .with_filter(filter::LevelFilter::INFO),
        )
        .init();

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

    let handle = Handle::new();
    tokio::spawn(graceful_shutdown(handle.clone()));

    let port = parse_port_or_default("HTTPS_PORT", 3000);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    let tls_config = RustlsConfig::from_pem_file(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("self_signed_certs/cert.pem"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("self_signed_certs/key.pem"),
    )
    .await
    .unwrap();

    let jwt_secret =
        env::var("JWT_SECRET").expect("The environment variable 'JWT_SECRET' must be set");

    let conn = Connection::open(get_database_path_from_args()).unwrap();
    let app_config = AppConfig::new(conn, jwt_secret);

    tracing::info!("HTTPS server listening on {}", addr);
    axum_server::bind_rustls(addr, tls_config)
        .handle(handle)
        .serve(
            build_router()
                .with_state(app_config)
                .layer(tracing_layer)
                .into_make_service(),
        )
        .await
        .unwrap();
}

fn get_database_path_from_args() -> PathBuf {
    let args: Vec<String> = args().collect();

    if args.len() < 2 {
        let program_name = args[0].clone();
        eprintln!("Usage: {program_name:?} <database_path>");
        exit(1);
    }

    PathBuf::from(&args[1])
}
