//! This crate provides an acceptor implementing `roa_core::Accept` and an app extension.
//!
//! ### TcpIncoming
//!
//! ```
//! use roa_core::{App, Context, Result};
//! use roa_tcp::TcpIncoming;
//! use std::io;
//!
//! async fn end(_ctx: &mut Context<()>) -> Result {
//!     Ok(())
//! }
//!
//! # fn main() -> io::Result<()> {
//! let app = App::new(()).end(end);
//! let incoming = TcpIncoming::bind("127.0.0.1:0")?;
//! let server = app.accept(incoming);
//! // server.await
//! Ok(())
//! # }
//! ```
//!
//! ### Listener
//!
//! ```
//! use roa_core::{App, Context, Result};
//! use roa_tcp::Listener;
//! use std::io;
//!
//! async fn end(_ctx: &mut Context<()>) -> Result {
//!     Ok(())
//! }
//!
//! # fn main() -> io::Result<()> {
//! let app = App::new(()).end(end);
//! let (addr, server) = app.bind("127.0.0.1:0")?;
//! // server.await
//! Ok(())
//! # }
//! ```

#![warn(missing_docs)]

mod incoming;
mod listen;

#[doc(inline)]
pub use incoming::{TcpIncoming, WrapStream};

#[doc(inline)]
pub use listen::Listener;
