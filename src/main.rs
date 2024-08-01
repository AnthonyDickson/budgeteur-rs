use std::{env, net::SocketAddr, path::PathBuf};

use axum_server::{tls_rustls::RustlsConfig, Handle};
use tracing;
use tracing_subscriber::{filter, layer::SubscriberExt, util::SubscriberInitExt, Layer};

use backrooms_rs::{build_router, graceful_shutdown, parse_port_or_default, AppConfig};

// TODO: Add route for creating user (email + password). Hash passwords with a salt which is stored alongside the hashed and salted password.
#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .pretty()
                .with_filter(filter::LevelFilter::INFO),
        )
        .init();

    let handle = Handle::new();
    tokio::spawn(graceful_shutdown(handle.clone()));

    let port = parse_port_or_default("HTTPS_PORT", 3000);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    let tls_config = RustlsConfig::from_pem_file(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("self_signed_certs")
            .join("../self_signed_certs/cert.pem"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("self_signed_certs")
            .join("../self_signed_certs/key.pem"),
    )
    .await
    .unwrap();

    let jwt_secret =
        env::var("JWT_SECRET").expect("The environment variable 'JWT_SECRET' must be set.");

    // TODO: Load database into memory or create it if it does not exist.
    // TODO: Add database connection to app state.
    let app_config = AppConfig::new(jwt_secret);

    tracing::info!("HTTPS server listening on {}", addr);
    axum_server::bind_rustls(addr, tls_config)
        .handle(handle)
        .serve(build_router().with_state(app_config).into_make_service())
        .await
        .unwrap();
}
