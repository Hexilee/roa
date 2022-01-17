//! RUST_LOG=info Cargo run --example https,
//! then request https://127.0.0.1:8000.

use std::error::Error as StdError;
use std::fs::File;
use std::io::BufReader;

use log::info;
use roa::body::DispositionType;
use roa::logger::logger;
use roa::preload::*;
use roa::tls::pemfile::{certs, rsa_private_keys};
use roa::tls::{Certificate, PrivateKey, ServerConfig, TlsListener};
use roa::{App, Context};
use tracing_subscriber::EnvFilter;

async fn serve_file(ctx: &mut Context) -> roa::Result {
    ctx.write_file("assets/welcome.html", DispositionType::Inline)
        .await
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init()
        .map_err(|err| anyhow::anyhow!("fail to init tracing subscriber: {}", err))?;

    let mut cert_file = BufReader::new(File::open("assets/cert.pem")?);
    let mut key_file = BufReader::new(File::open("assets/key.pem")?);
    let cert_chain = certs(&mut cert_file)?
        .into_iter()
        .map(Certificate)
        .collect();

    let config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(
            cert_chain,
            PrivateKey(rsa_private_keys(&mut key_file)?.remove(0)),
        )?;

    let app = App::new().gate(logger).end(serve_file);
    app.listen_tls("127.0.0.1:8000", config, |addr| {
        info!("Server is listening on https://localhost:{}", addr.port())
    })?
    .await?;
    Ok(())
}
