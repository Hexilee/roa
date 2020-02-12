//! [![Build status](https://img.shields.io/travis/Hexilee/roa/master.svg)](https://travis-ci.org/Hexilee/roa)
//! [![codecov](https://codecov.io/gh/Hexilee/roa/branch/master/graph/badge.svg)](https://codecov.io/gh/Hexilee/roa)
//! [![Rust Docs](https://docs.rs/roa-core/badge.svg)](https://docs.rs/roa-core)
//! [![Crate version](https://img.shields.io/crates/v/roa-core.svg)](https://crates.io/crates/roa-core)
//! [![Download](https://img.shields.io/crates/d/roa-core.svg)](https://crates.io/crates/roa-core)
//! [![Version](https://img.shields.io/badge/rustc-1.39+-lightgray.svg)](https://blog.rust-lang.org/2019/11/07/Rust-1.39.0.html)
//! [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/Hexilee/roa/blob/master/LICENSE)
//!
//! ### Introduction
//!
//! Roa is an async web framework inspired by koajs, lightweight but powerful.
//!
//! ### Application
//!
//! A Roa application is a structure containing a middleware group which composes and executes middleware functions in a stack-like manner.
//!
//! The obligatory hello world application:
//!
//! ```rust,no_run
//! use roa_core::App;
//! use log::info;
//! use std::error::Error as StdError;
//!
//! #[async_std::main]
//! async fn main() -> Result<(), Box<dyn StdError>> {
//!     let mut app = App::new(());
//!     app.end(|ctx| async move {
//!         ctx.resp_mut().await.write_str("Hello, World");
//!         Ok(())
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
//!
//! The following example responds with "Hello World", however, the request flows through
//! the `logging` middleware to mark when the request started, then continue
//! to yield control through the response middleware. When a middleware invokes `next().await`
//! the function suspends and passes control to the next middleware defined. After there are no more
//! middleware to execute downstream, the stack will unwind and each middleware is resumed to perform
//! its upstream behaviour.
//!
//! ```rust,no_run
//! use roa_core::App;
//! use log::info;
//! use std::error::Error as StdError;
//! use std::time::Instant;
//!
//! #[async_std::main]
//! async fn main() -> Result<(), Box<dyn StdError>> {
//!     let mut app = App::new(());
//!     app.gate_fn(|_ctx, next| async move {
//!         let inbound = Instant::now();
//!         next().await?;
//!         info!("time elapsed: {} ms", inbound.elapsed().as_millis());
//!         Ok(())
//!     });
//!
//!     app.end(|ctx| async move {
//!         ctx.resp_mut().await.write_str("Hello, World");
//!         Ok(())
//!     });
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
//! You can catch or straightly throw a Error returned by next.
//!
//! ```rust
//! use roa_core::{App, throw};
//! use async_std::task::spawn;
//! use http::StatusCode;
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let (addr, server) = App::new(())
//!         .gate_fn(|ctx, next| async move {
//!             // catch
//!             if let Err(err) = next().await {
//!                 // teapot is ok
//!                 if err.status_code != StatusCode::IM_A_TEAPOT {
//!                     return Err(err)
//!                 }
//!             }
//!             Ok(())
//!         })
//!         .gate_fn(|ctx, next| async move {
//!             next().await?; // just throw
//!             unreachable!()
//!         })
//!         .end(|_ctx| async move {
//!             throw!(StatusCode::IM_A_TEAPOT, "I'm a teapot!")
//!         })
//!         .run_local()?;
//!     spawn(server);
//!     let resp = reqwest::get(&format!("http://{}", addr)).await?;
//!     assert_eq!(StatusCode::OK, resp.status());
//!     Ok(())
//! }
//! ```
//!
//! #### error_handler
//! App has a error_handler to handle `Error` thrown by the top middleware.
//! This is the error_handler:
//!
//! ```rust
//! use roa_core::{Context, Error, Result, Model, ErrorKind};
//! pub async fn error_handler<M: Model>(context: Context<M>, err: Error) -> Result {
//!     context.resp_mut().await.status = err.status_code;
//!     if err.expose {
//!         context.resp_mut().await.write_str(&err.message);
//!     }
//!     if err.kind == ErrorKind::ServerError {
//!         Err(err)
//!     } else {
//!         Ok(())
//!     }
//! }
//! ```
//!
//! The Error thrown by this error_handler will be handled by hyper.
#![warn(missing_docs)]

mod app;
mod body;
mod context;
mod err;
mod group;
mod middleware;
mod model;
mod next;
mod request;
mod response;
pub(crate) use app::AddrStream;

#[doc(inline)]
pub use app::App;

#[doc(inline)]
pub use body::{Body, Callback as BodyCallback};

#[doc(inline)]
pub use context::{Bucket, Context, Variable};

#[doc(inline)]
pub use err::{Error, ErrorKind, Result, ResultFuture};

#[doc(inline)]
pub use middleware::Middleware;

#[doc(inline)]
pub use group::{join, join_all};

#[doc(inline)]
pub use model::{Model, State};
pub(crate) use next::last;

#[doc(inline)]
pub use next::Next;

#[doc(inline)]
pub use request::Request;

#[doc(inline)]
pub use response::Response;

pub use http::{header, StatusCode};

pub use async_trait::async_trait;
