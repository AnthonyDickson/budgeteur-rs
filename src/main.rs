use std::{env, net::SocketAddr, path::PathBuf};

use axum_server::{tls_rustls::RustlsConfig, Handle};
use tracing;
use tracing_subscriber::{filter, layer::SubscriberExt, util::SubscriberInitExt, Layer};

use backrooms_rs::{
    build_router, graceful_shutdown, parse_port_or_default, redirect_http_to_https, AppConfig,
    Ports,
};

// TODO: Add route for creating user (email + password). Hash passwords with a salt which is stored alongside the hashed and salted password.
// TODO: Add route for login which issues a JWT on success (What happens on failure?).
// TODO: Add middleware that checks for valid JWT before providing access to protected routes.
#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .pretty()
                .with_filter(filter::LevelFilter::INFO),
        )
        .init();

    let ports = Ports {
        http: parse_port_or_default("HTTP_PORT", 3000),
        https: parse_port_or_default("HTTPS_PORT", 3001),
    };

    let jwt_secret =
        env::var("JWT_SECRET").expect("The environment variable 'JWT_SECRET' must be set.");

    let app_config = AppConfig { ports, jwt_secret };

    let app = build_router(app_config);

    // configure certificate and private key used by https
    let config = RustlsConfig::from_pem_file(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("self_signed_certs")
            .join("../self_signed_certs/cert.pem"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("self_signed_certs")
            .join("../self_signed_certs/key.pem"),
    )
    .await
    .unwrap();

    let handle = Handle::new();
    tokio::spawn(graceful_shutdown(handle.clone()));
    tokio::spawn(redirect_http_to_https(ports));

    let addr = SocketAddr::from(([127, 0, 0, 1], ports.https));
    tracing::info!("HTTPS server listening on {}", addr);
    axum_server::bind_rustls(addr, config)
        .handle(handle)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
