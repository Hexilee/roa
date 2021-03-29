//! This module provides an acceptor implementing `roa_core::Accept` and an app extension.
//!
//! ### TlsIncoming
//!
//! ```rust
//! use roa::{App, Context, Status};
//! use roa::tls::{TlsIncoming, ServerConfig, NoClientAuth};
//! use roa::tls::internal::pemfile::{certs, rsa_private_keys};
//! use std::fs::File;
//! use std::io::BufReader;
//!
//! async fn end(_ctx: &mut Context) -> Result<(), Status> {
//!     Ok(())
//! }
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut config = ServerConfig::new(NoClientAuth::new());
//! let mut cert_file = BufReader::new(File::open("../assets/cert.pem")?);
//! let mut key_file = BufReader::new(File::open("../assets/key.pem")?);
//! let cert_chain = certs(&mut cert_file).unwrap();
//! let mut keys = rsa_private_keys(&mut key_file).unwrap();
//! config.set_single_cert(cert_chain, keys.remove(0))?;
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
//! use roa::tls::{TlsIncoming, ServerConfig, NoClientAuth, TlsListener};
//! use roa::tls::internal::pemfile::{certs, rsa_private_keys};
//! use std::fs::File;
//! use std::io::BufReader;
//!
//! async fn end(_ctx: &mut Context) -> Result<(), Status> {
//!     Ok(())
//! }
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut config = ServerConfig::new(NoClientAuth::new());
//! let mut cert_file = BufReader::new(File::open("../assets/cert.pem")?);
//! let mut key_file = BufReader::new(File::open("../assets/key.pem")?);
//! let cert_chain = certs(&mut cert_file).unwrap();
//! let mut keys = rsa_private_keys(&mut key_file).unwrap();
//! config.set_single_cert(cert_chain, keys.remove(0))?;
//! let (addr, server) = App::new().end(end).bind_tls("127.0.0.1:0", config)?;
//! // server.await
//! Ok(())
//! # }
//! ```

#[doc(no_inline)]
pub use rustls::*;

mod incoming;

#[cfg(feature = "tcp")]
mod listener;

#[doc(inline)]
pub use incoming::TlsIncoming;
#[doc(inline)]
#[cfg(feature = "tcp")]
pub use listener::TlsListener;
