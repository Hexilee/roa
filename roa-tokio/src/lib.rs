#![cfg_attr(feature = "docs", doc = include_str!("../README.md"))]
#![cfg_attr(feature = "docs", warn(missing_docs))]

mod net;
mod runtime;

#[doc(inline)]
pub use net::TcpIncoming;
#[doc(inline)]
pub use runtime::Exec;
