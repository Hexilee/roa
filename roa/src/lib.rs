//! [![Build status](https://img.shields.io/travis/Hexilee/roa/master.svg)](https://travis-ci.org/Hexilee/roa)
//! [![codecov](https://codecov.io/gh/Hexilee/roa/branch/master/graph/badge.svg)](https://codecov.io/gh/Hexilee/roa)
//! [![Rust Docs](https://docs.rs/roa/badge.svg)](https://docs.rs/roa)
//! [![Crate version](https://img.shields.io/crates/v/roa.svg)](https://crates.io/crates/roa)
//! [![Download](https://img.shields.io/crates/d/roa.svg)](https://crates.io/crates/roa)
//! [![Version](https://img.shields.io/badge/rustc-1.39+-lightgray.svg)](https://blog.rust-lang.org/2019/11/07/Rust-1.39.0.html)
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
//! use roa::App;
//! use roa::preload::*;
//! use log::info;
//! use std::error::Error as StdError;
//!
//! #[async_std::main]
//! async fn main() -> Result<(), Box<dyn StdError>> {
//!     let mut app = App::new(());
//!     app.end(|mut ctx| async move {
//!         ctx.write_text("Hello, World")
//!     });
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
//! use roa::App;
//! use roa::preload::*;
//! use log::info;
//! use std::error::Error as StdError;
//! use std::time::Instant;
//!
//! #[async_std::main]
//! async fn main() -> Result<(), Box<dyn StdError>> {
//!     let mut app = App::new(());
//!     // logger
//!     app.gate_fn(|ctx, next| async move {
//!       next.await?;
//!       let rt = ctx.resp().must_get("x-response-time")?;
//!       info!("{} {} - {}", ctx.method(), ctx.uri(), rt);
//!       Ok(())
//!     });
//!
//!     // x-response-time
//!     app.gate_fn(|mut ctx, next| async move {
//!         let start = Instant::now();
//!         next.await?;
//!         let ms = start.elapsed().as_millis();
//!         ctx.resp_mut().insert("x-response-time", format!("{}ms", ms))?;
//!         Ok(())
//!     });
//!
//!     // response
//!     app.end(|mut ctx| async move {
//!         ctx.write_text("Hello, World")
//!     });
//!
//!     app.listen("127.0.0.1:8000", |addr| {
//!         info!("Server is listening on {}", addr)
//!     })?
//!     .await?;
//!     Ok(())
//! }
//! ```
//!
//! ### Error Handling
//!
//! You can catch or straightly throw an error returned by next.
//!
//! ```rust,no_run
//! use roa::{App, throw};
//! use roa::preload::*;
//! use roa::http::StatusCode;
//! use async_std::task::spawn;
//! use log::info;
//!
//! #[async_std::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     App::new(())
//!         .gate_fn(|ctx, next| async move {
//!             // catch
//!             if let Err(err) = next.await {
//!                 // teapot is ok
//!                 if err.status_code != StatusCode::IM_A_TEAPOT {
//!                     return Err(err)
//!                 }
//!             }
//!             Ok(())
//!         })
//!         .gate_fn(|ctx, next| async move {
//!             next.await?; // just throw
//!             unreachable!()
//!         })
//!         .end(|_ctx| async move {
//!             throw!(StatusCode::IM_A_TEAPOT, "I'm a teapot!")
//!         })
//!         .listen("127.0.0.1:8000", |addr| {
//!             info!("Server is listening on {}", addr)
//!         })?
//!         .await?;
//!     Ok(())
//! }
//! ```
//!
//! #### error_handler
//! App has an error_handler to handle error thrown by the top middleware.
//! This is the error_handler:
//!
//! ```rust,no_run
//! use roa_core::{Context, Error, Result, State, ErrorKind};
//! pub async fn error_handler<S: State>(mut context: Context<S>, err: Error) -> Result {
//!     // set status code to err.status_code.
//!     context.resp_mut().status = err.status_code;
//!     if err.expose {
//!         // write err.message to response body if err.expose.
//!         context.resp_mut().write_str(&err.message);
//!     }
//!     if err.kind == ErrorKind::ServerError {
//!         // thrown to hyper
//!         Err(err)
//!     } else {
//!         // caught
//!         Ok(())
//!     }
//! }
//! ```
//!
//! The error thrown by this error_handler will be handled by hyper.
//!
//! ### Router.
//! Roa provides a configurable and nestable router.
//!
//! ```rust,no_run
//! use roa::preload::*;
//! use roa::router::Router;
//! use roa::App;
//! use async_std::task::spawn;
//! use log::info;
//!
//! #[async_std::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut router = Router::<()>::new();
//!     // get dynamic "/:id"
//!     router.get("/:id", |ctx| async move {
//!         let id: u64 = ctx.must_param("id")?.parse()?;
//!         // do something
//!         Ok(())
//!     });
//!     App::new(())
//!         // route with prefix "/user"
//!         .gate(router.routes("/user")?)
//!         .listen("127.0.0.1:8000", |addr| {
//!             info!("Server is listening on {}", addr)
//!         })?
//!         .await?;
//!     
//!     // get "/user/1", then id == 1.
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
//! use roa::App;
//! use async_std::task::spawn;
//! use log::info;
//!
//! #[async_std::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     App::new(())
//!         .gate(query_parser)
//!         .end( |ctx| async move {
//!             let id: u64 = ctx.must_query("id")?.parse()?;
//!             Ok(())
//!         })
//!         .listen("127.0.0.1:8080", |addr| {
//!             info!("Server is listening on {}", addr)
//!         })?
//!         .await?;     
//!     // request "/?id=1", then id == 1.
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

pub use roa_core::*;

#[cfg(feature = "body")]
pub use roa_body as body;

#[cfg(feature = "websocket")]
pub use roa_juniper as juniper;

#[cfg(feature = "router")]
pub use roa_router as router;

#[cfg(feature = "tcp")]
pub use roa_tcp as tcp;

#[cfg(feature = "tls")]
pub use roa_tls as tls;

#[cfg(feature = "websocket")]
pub use roa_websocket as websocket;

pub mod cors;
pub mod forward;
pub mod header;
pub mod logger;
pub mod query;

#[cfg(feature = "cookies")]
pub mod cookie;

#[cfg(feature = "jwt")]
pub mod jwt;

#[cfg(feature = "compress")]
pub mod compress;

/// Reexport all extensional traits.
pub mod preload {
    pub use crate::forward::Forward;
    pub use crate::header::FriendlyHeaders;
    pub use crate::query::Query;

    #[cfg(feature = "tcp")]
    pub use crate::tcp::Listener;

    #[cfg(feature = "tls")]
    pub use crate::tls::TlsListener;

    #[cfg(feature = "body")]
    pub use crate::body::PowerBody;

    #[cfg(feature = "cookies")]
    pub use crate::cookie::{CookieGetter, CookieSetter};

    #[cfg(feature = "jwt")]
    pub use crate::jwt::JwtVerifier;

    #[cfg(feature = "router")]
    pub use crate::router::RouterParam;
}
