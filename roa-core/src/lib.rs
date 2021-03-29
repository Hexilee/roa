#![cfg_attr(feature = "docs", feature(doc_cfg, external_doc))]
#![cfg_attr(feature = "docs", doc(include = "../README.md"))]
#![cfg_attr(feature = "docs", warn(missing_docs))]

mod app;
mod body;
mod context;
mod err;
mod executor;
mod group;
mod middleware;
mod request;
mod response;
mod state;

#[doc(inline)]
pub use app::{AddrStream, App};
pub use async_trait::async_trait;
#[doc(inline)]
pub use body::Body;
#[doc(inline)]
pub use context::{Context, Variable};
#[doc(inline)]
pub use err::{Result, Status};
#[doc(inline)]
pub use executor::{Executor, JoinHandle, Spawn};
#[doc(inline)]
pub use group::{Boxed, Chain, EndpointExt, MiddlewareExt, Shared};
pub use http;
pub use hyper::server::accept::Accept;
pub use hyper::server::Server;
#[doc(inline)]
pub use middleware::{Endpoint, Middleware, Next};
#[doc(inline)]
pub use request::Request;
#[doc(inline)]
pub use response::Response;
#[doc(inline)]
pub use state::State;
