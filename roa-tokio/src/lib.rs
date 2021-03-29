#![cfg_attr(feature = "docs", feature(external_doc))]
#![cfg_attr(feature = "docs", doc(include = "../README.md"))]
#![cfg_attr(feature = "docs", warn(missing_docs))]

mod net;
mod runtime;

#[doc(inline)]
pub use net::TcpIncoming;
#[doc(inline)]
pub use runtime::Exec;
