//! [![Build status](https://img.shields.io/travis/Hexilee/roa/master.svg)](https://travis-ci.org/Hexilee/roa)
//! [![codecov](https://codecov.io/gh/Hexilee/roa/branch/master/graph/badge.svg)](https://codecov.io/gh/Hexilee/roa)
//! [![Rust Docs](https://docs.rs/roa/badge.svg)](https://docs.rs/roa)
//! [![Crate version](https://img.shields.io/crates/v/roa.svg)](https://crates.io/crates/roa)
//! [![Download](https://img.shields.io/crates/d/roa.svg)](https://crates.io/crates/roa)
//! [![Version](https://img.shields.io/badge/rustc-1.40+-lightgray.svg)](https://blog.rust-lang.org/2019/12/19/Rust-1.40.0.html)
//! [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/Hexilee/roa/blob/master/LICENSE)
//!
//! ### Introduction
//!
//! Roa is an async web framework inspired by koajs, lightweight but powerful.
//!
//! ### Application
//!
//! A Roa application is a structure containing a middleware group
//! which composes and executes middleware functions in a stack-like manner.
//!
//! The obligatory hello world application:
//!
//! ```rust,no_run
//! use roa::{App, Context};
//! use roa::preload::*;
//! use log::info;
//! use std::error::Error as StdError;
//!
//! async fn end(ctx: &mut Context<()>) -> roa::Result {
//!     ctx.write_text("Hello, World");
//!     Ok(())
//! }
//!
//! #[async_std::main]
//! async fn main() -> Result<(), Box<dyn StdError>> {
//!     let app = App::new(()).end(end);
//!     app.listen("127.0.0.1:8000", |addr| {
//!         info!("Server is listening on {}", addr)
//!     })?
//!     .await?;
//!     Ok(())
//! }
//! ```
//!
//! #### Cascading
//! Like koajs, middleware suspends and passes control to "downstream" by invoking `next.await`.
//! Then control flows back "upstream" when `next.await` returns.
//!
//! The following example responds with "Hello World",
//! however first the request flows through the x-response-time and logging middleware to mark
//! when the request started, then continue to yield control through the response middleware.
//! When a middleware invokes next the function suspends and passes control to the next middleware defined.
//! After there are no more middleware to execute downstream,
//! the stack will unwind and each middleware is resumed to perform its upstream behaviour.
//!
//! ```rust,no_run
//! use roa::{App, Context, Next};
//! use roa::preload::*;
//! use log::info;
//! use std::error::Error as StdError;
//! use std::time::Instant;
//!
//! #[async_std::main]
//! async fn main() -> Result<(), Box<dyn StdError>> {
//!     let app = App::new(())
//!         .gate(logger)
//!         .gate(x_response_time)
//!         .end(response);
//!     app.listen("127.0.0.1:8000", |addr| {
//!         info!("Server is listening on {}", addr)
//!     })?
//!     .await?;
//!     Ok(())
//! }
//!
//! async fn logger(ctx: &mut Context<()>, next: Next<'_>) -> roa::Result {
//!     next.await?;
//!     let rt = ctx.resp.must_get("x-response-time")?;
//!     info!("{} {} - {}", ctx.method(), ctx.uri(), rt);
//!     Ok(())
//! }
//!
//! async fn x_response_time(ctx: &mut Context<()>, next: Next<'_>) -> roa::Result {
//!     let start = Instant::now();
//!     next.await?;
//!     let ms = start.elapsed().as_millis();
//!     ctx.resp.insert("x-response-time", format!("{}ms", ms))?;
//!     Ok(())
//! }
//!
//! async fn response(ctx: &mut Context<()>) -> roa::Result {
//!     ctx.write_text("Hello, World");
//!     Ok(())
//! }
//!
//! ```
//!
//! ### Error Handling
//!
//! You can catch or straightly throw an error returned by next.
//!
//! ```rust,no_run
//! use roa::{App, Context, Next, throw};
//! use roa::preload::*;
//! use roa::http::StatusCode;
//! use async_std::task::spawn;
//! use log::info;
//!
//! #[async_std::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let app = App::new(())
//!         .gate(catch)
//!         .gate(not_catch)
//!         .end(error);
//!     app.listen("127.0.0.1:8000", |addr| {
//!         info!("Server is listening on {}", addr)
//!     })?
//!     .await?;
//!     Ok(())
//! }
//!
//! async fn catch(_ctx: &mut Context<()>, next: Next<'_>) -> roa::Result {
//!     // catch
//!     if let Err(status) = next.await {
//!         // teapot is ok
//!         if status.status_code != StatusCode::IM_A_TEAPOT {
//!             return Err(status);
//!         }
//!     }
//!     Ok(())
//! }
//!
//! async fn not_catch(ctx: &mut Context<()>, next: Next<'_>) -> roa::Result {
//!     next.await?; // just throw
//!     unreachable!()
//! }
//!
//! async fn error(ctx: &mut Context<()>) -> roa::Result {
//!     throw!(StatusCode::IM_A_TEAPOT, "I'm a teapot!")
//! }
//!
//! ```
//!
//! #### status_handler
//! App has an status_handler to handle status thrown by the top middleware.
//! This is the status_handler:
//!
//! ```rust,no_run
//! use roa::{Context, Status, Result, State};
//! pub async fn status_handler<S: State>(ctx: &mut Context<S>, status: Status) -> Result {
//!     ctx.resp.status = status.status_code;
//!     if status.expose {
//!         ctx.resp.write(status.message.clone());
//!     }
//!     if status.status_code.as_u16() / 100 == 5 {
//!         // internal server error, throw to hyper
//!         Err(status)
//!     } else {
//!         Ok(())
//!     }
//! }
//! ```
//!
//! The status thrown by this status_handler will be handled by hyper.
//!
//! ### Router.
//! Roa provides a configurable and nestable router.
//!
//! ```rust,no_run
//! use roa::preload::*;
//! use roa::router::{Router, get};
//! use roa::{App, Context};
//! use async_std::task::spawn;
//! use log::info;
//!
//! #[async_std::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let router = Router::new()
//!         .on("/:id", get(end)); // get dynamic "/:id"
//!     let app = App::new(())
//!         .end(router.routes("/user")?); // route with prefix "/user"
//!     app.listen("127.0.0.1:8000", |addr| {
//!         info!("Server is listening on {}", addr)
//!     })?
//!     .await?;
//!     
//!     Ok(())
//! }
//!
//! async fn end(ctx: &mut Context<()>) -> roa::Result {
//!     // get "/user/1", then id == 1.
//!     let id: u64 = ctx.must_param("id")?.parse()?;
//!     // do something
//!     Ok(())
//! }
//! ```
//!
//! ### Query
//!
//! Roa provides a middleware `query_parser`.
//!
//! ```rust,no_run
//! use roa::preload::*;
//! use roa::query::query_parser;
//! use roa::{App, Context};
//! use async_std::task::spawn;
//! use log::info;
//!
//! async fn must(ctx: &mut Context<()>) -> roa::Result {
//!     // request "/?id=1", then id == 1.
//!     let id: u64 = ctx.must_query("id")?.parse()?;
//!     Ok(())
//! }
//!
//! #[async_std::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let app = App::new(())
//!         .gate(query_parser)
//!         .end(must);
//!     app.listen("127.0.0.1:8080", |addr| {
//!         info!("Server is listening on {}", addr)
//!     })?
//!     .await?;     
//!     Ok(())
//! }
//! ```
//!
//! ### Other modules
//!
//! - body: dealing with body more conviniently.
//! - compress: supports transparent content compression.
//! - cors: CORS support.
//! - forward: "X-Forwarded-*" parser.
//! - header: dealing with headers more conviniently.
//! - jwt: json web token support.
//! - logger: a logger middleware.

#![warn(missing_docs)]
#![cfg_attr(feature = "docs", feature(doc_cfg))]

pub use roa_core::*;

#[cfg(feature = "router")]
#[cfg_attr(feature = "docs", doc(cfg(feature = "router")))]
pub mod router;

#[cfg(feature = "tcp")]
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

#[cfg(feature = "compress")]
#[cfg_attr(feature = "docs", doc(cfg(feature = "compress")))]
pub mod compress;

pub mod body;
pub mod cors;
pub mod forward;
pub mod header;
pub mod logger;
pub mod query;

/// Reexport all extensional traits.
pub mod preload {
    pub use crate::body::PowerBody;
    pub use crate::forward::Forward;
    pub use crate::header::FriendlyHeaders;
    pub use crate::query::Query;

    #[cfg(feature = "tcp")]
    #[doc(no_inline)]
    pub use crate::tcp::Listener;

    #[cfg(feature = "tls")]
    pub use crate::tls::TlsListener;

    #[cfg(feature = "cookies")]
    pub use crate::cookie::{CookieGetter, CookieSetter};

    #[cfg(feature = "jwt")]
    pub use crate::jwt::JwtVerifier;

    #[cfg(feature = "router")]
    pub use crate::router::RouterParam;
}
