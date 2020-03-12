//! [![Stable Test](https://github.com/Hexilee/roa/workflows/Stable%20Test/badge.svg)](https://github.com/Hexilee/roa/actions)
//! [![codecov](https://codecov.io/gh/Hexilee/roa/branch/master/graph/badge.svg)](https://codecov.io/gh/Hexilee/roa)
//! [![Rust Docs](https://docs.rs/roa-core/badge.svg)](https://docs.rs/roa-core)
//! [![Crate version](https://img.shields.io/crates/v/roa-core.svg)](https://crates.io/crates/roa-core)
//! [![Download](https://img.shields.io/crates/d/roa-core.svg)](https://crates.io/crates/roa-core)
//! [![Version](https://img.shields.io/badge/rustc-1.40+-lightgray.svg)](https://blog.rust-lang.org/2019/12/19/Rust-1.40.0.html)
//! [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/Hexilee/roa/blob/master/LICENSE)
//!
//! ### Introduction
//!
//! Core components of Roa framework.
//!
//! If you are new to roa, please go to the documentation of roa framework.
//!
//! ### Application
//!
//! A Roa application is a structure containing a middleware group which composes and executes middleware functions in a stack-like manner.
//!
//! The obligatory hello world application:
//!
//! ```rust
//! use roa_core::{App, Context, Result, Error};
//!
//! let app = App::new((), end);
//! async fn end(ctx: &mut Context<()>) -> Result {
//!     ctx.resp.write("Hello, World");
//!     Ok(())
//! }
//! ```
//!
//! #### Cascading
//!
//! The following example responds with "Hello World", however, the request flows through
//! the `logging` middleware to mark when the request started, then continue
//! to yield control through the response middleware. When a middleware invokes `next.await`
//! the function suspends and passes control to the next middleware defined. After there are no more
//! middleware to execute downstream, the stack will unwind and each middleware is resumed to perform
//! its upstream behaviour.
//!
//! ```rust
//! use roa_core::{App, Context, Result, Error, MiddlewareExt, Next};
//! use std::time::Instant;
//! use log::info;
//!
//! let app = App::new((), gate.end(end));
//!
//! async fn end(ctx: &mut Context<()>) -> Result {
//!     ctx.resp.write("Hello, World");
//!     Ok(())
//! }
//!
//! async fn gate(ctx: &mut Context<()>, next: Next<'_>) -> Result {
//!     let inbound = Instant::now();
//!     next.await?;
//!     info!("time elapsed: {} ms", inbound.elapsed().as_millis());
//!     Ok(())
//! }
//! ```
//!
//! ### Error Handling
//!
//! You can catch or straightly throw an Error returned by next.
//!
//! ```rust
//! use roa_core::{App, Context, Result, Error, MiddlewareExt, Next, throw};
//! use roa_core::http::StatusCode;
//!         
//! let mut app = App::new((), catch.chain(gate).end(end));
//!
//! async fn catch(ctx: &mut Context<()>, next: Next<'_>) -> Result {
//!     // catch
//!     if let Err(err) = next.await {
//!         // teapot is ok
//!         if err.status_code != StatusCode::IM_A_TEAPOT {
//!             return Err(err)
//!         }
//!     }
//!     Ok(())
//! }
//! async fn gate(ctx: &mut Context<()>, next: Next<'_>) -> Result {
//!     next.await?; // just throw
//!     unreachable!()
//! }
//!
//! async fn end(ctx: &mut Context<()>) -> Result {
//!     throw!(StatusCode::IM_A_TEAPOT, "I'm a teapot!")
//! }
//! ```
//!
//! #### error_handler
//! App has an error_handler to handle `Error` thrown by the top middleware.
//! This is the error_handler:
//!
//! ```rust
//! use roa_core::{Context, Error, Result, ErrorKind, State};
//! pub async fn error_handler<S: State>(context: &mut Context<S>, err: Error) -> Result {
//!     context.resp.status = err.status_code;
//!     if err.expose {
//!         context.resp.write(err.message.clone());
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
//!
//! ### HTTP Server.
//!
//! Use `roa_core::accept` to construct a http server.
//! Please refer to crate [![Crate version](https://img.shields.io/crates/v/roa-tcp.svg)](https://crates.io/crates/roa-tcp) for more information.

#![warn(missing_docs)]

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

#[doc(inline)]
pub use executor::{BlockingObj, Executor, FutureObj, JoinHandle, Spawn};

#[doc(inline)]
pub use context::{Context, Variable};

#[doc(inline)]
pub use err::{Error, ErrorKind, Result, ResultFuture};

#[doc(inline)]
pub use middleware::{Endpoint, Middleware, Next};

#[doc(inline)]
pub use group::{Boxed, Chain, EndpointExt, MiddlewareExt, Shared};

pub use state::State;

#[doc(inline)]
pub use request::Request;

#[doc(inline)]
pub use response::Response;

#[doc(inline)]
pub use body::Body;

pub use http;

pub use hyper::server::{accept::Accept, Server};

pub use async_trait::async_trait;
