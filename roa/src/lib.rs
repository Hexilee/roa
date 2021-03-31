#![cfg_attr(feature = "docs", feature(doc_cfg, external_doc))]
#![cfg_attr(feature = "docs", doc(include = "../README.md"))]
#![cfg_attr(feature = "docs", warn(missing_docs))]

pub use roa_core::*;

#[cfg(feature = "router")]
#[cfg_attr(feature = "docs", doc(cfg(feature = "router")))]
pub mod router;

#[cfg(feature = "tcp")]
#[cfg_attr(feature = "docs", doc(cfg(feature = "tcp")))]
pub mod tcp;

#[cfg(feature = "tls")]
#[cfg_attr(feature = "docs", doc(cfg(feature = "tls")))]
pub mod tls;

#[cfg(feature = "websocket")]
#[cfg_attr(feature = "docs", doc(cfg(feature = "websocket")))]
pub mod websocket;

#[cfg(feature = "cookies")]
#[cfg_attr(feature = "docs", doc(cfg(feature = "cookies")))]
pub mod cookie;

#[cfg(feature = "jwt")]
#[cfg_attr(feature = "docs", doc(cfg(feature = "jwt")))]
pub mod jwt;

#[cfg(feature = "donnot-compile-me-until-async-compression-upgrade-bytes-version")]
#[cfg_attr(feature = "docs", doc(cfg(feature = "compress")))]
pub mod compress;

pub mod body;
pub mod cors;
pub mod forward;
pub mod logger;
pub mod query;
pub mod stream;

/// Reexport all extension traits.
pub mod preload {
    pub use crate::body::PowerBody;
    #[cfg(feature = "cookies")]
    pub use crate::cookie::{CookieGetter, CookieSetter};
    pub use crate::forward::Forward;
    #[cfg(feature = "jwt")]
    pub use crate::jwt::JwtVerifier;
    pub use crate::query::Query;
    #[cfg(feature = "router")]
    pub use crate::router::RouterParam;
    #[cfg(feature = "tcp")]
    #[doc(no_inline)]
    pub use crate::tcp::Listener;
    #[cfg(all(feature = "tcp", feature = "tls"))]
    #[doc(no_inline)]
    pub use crate::tls::TlsListener;
}
