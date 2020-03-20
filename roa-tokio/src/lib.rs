//! Roa-tokio is a crate to provide tokio-based runtime and acceptor for roa.

#![warn(missing_docs)]

#[cfg(doctest)]
doc_comment::doctest!("./README.md");

mod net;
mod runtime;

#[doc(inline)]
pub use net::TcpIncoming;

#[doc(inline)]
pub use runtime::Exec;
