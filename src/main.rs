use std::{env, net::SocketAddr, path::PathBuf};

use axum_server::{Handle, tls_rustls::RustlsConfig};
use tracing;
use tracing_subscriber::{filter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

use backrooms_rs::{build_app, graceful_shutdown, parse_port_or_default, Ports, redirect_http_to_https};

// TODO: Add route for creating user (email + password). Hash passwords with a salt which is stored alongside the hashed and salted password.
// TODO: Add route for login which issues a JWT on success (What happens on failure?).
// TODO: Add middleware that checks for valid JWT before providing access to protected routes.
#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .pretty()
                .with_filter(filter::LevelFilter::DEBUG)
        )
        .init();

    let ports = Ports {
        http: parse_port_or_default("HTTP_PORT", 3000),
        https: parse_port_or_default("HTTPS_PORT", 3001),
    };

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
        .serve(build_app().into_make_service())
        .await
        .unwrap();
}
