//! This module provides an acceptor implementing `roa_core::Accept` and an app extension.
//!
//! ### TlsIncoming
//!
//! ```rust
//! use roa::{App, Context, Status};
//! use roa::tls::{TlsIncoming, ServerConfig, Certificate, PrivateKey};
//! use roa::tls::pemfile::{certs, rsa_private_keys};
//! use std::fs::File;
//! use std::io::BufReader;
//!
//! async fn end(_ctx: &mut Context) -> Result<(), Status> {
//!     Ok(())
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut cert_file = BufReader::new(File::open("../assets/cert.pem")?);
//! let mut key_file = BufReader::new(File::open("../assets/key.pem")?);
//! let cert_chain = certs(&mut cert_file)?.into_iter().map(Certificate).collect();
//!
//! let config = ServerConfig::builder()
//!     .with_safe_defaults()
//!     .with_no_client_auth()
//!     .with_single_cert(cert_chain, PrivateKey(rsa_private_keys(&mut key_file)?.remove(0)))?;
//!
//! let incoming = TlsIncoming::bind("127.0.0.1:0", config)?;
//! let server = App::new().end(end).accept(incoming);
//! // server.await
//! Ok(())
//! # }
//! ```
//!
//! ### TlsListener
//!
//! ```rust
//! use roa::{App, Context, Status};
//! use roa::tls::{ServerConfig, TlsListener, Certificate, PrivateKey};
//! use roa::tls::pemfile::{certs, rsa_private_keys};
//! use std::fs::File;
//! use std::io::BufReader;
//!
//! async fn end(_ctx: &mut Context) -> Result<(), Status> {
//!     Ok(())
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut cert_file = BufReader::new(File::open("../assets/cert.pem")?);
//! let mut key_file = BufReader::new(File::open("../assets/key.pem")?);
//! let cert_chain = certs(&mut cert_file)?.into_iter().map(Certificate).collect();
//!
//! let config = ServerConfig::builder()
//!     .with_safe_defaults()
//!     .with_no_client_auth()
//!     .with_single_cert(cert_chain, PrivateKey(rsa_private_keys(&mut key_file)?.remove(0)))?;
//! let (addr, server) = App::new().end(end).bind_tls("127.0.0.1:0", config)?;
//! // server.await
//! Ok(())
//! # }
//! ```

#[doc(no_inline)]
pub use rustls::*;
#[doc(no_inline)]
pub use rustls_pemfile as pemfile;

mod incoming;

#[cfg(feature = "tcp")]
mod listener;

#[doc(inline)]
pub use incoming::TlsIncoming;
#[doc(inline)]
#[cfg(feature = "tcp")]
pub use listener::TlsListener;
