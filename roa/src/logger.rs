//! This module provides a middleware `logger`.
//!
//! ### Example
//!
//! ```rust
//! use roa::logger::logger;
//! use roa::preload::*;
//! use roa::App;
//! use roa::http::StatusCode;
//! use tokio::task::spawn;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     pretty_env_logger::init();
//!     let app = App::new()
//!         .gate(logger)
//!         .end("Hello, World");
//!     let (addr, server) = app.run()?;
//!     spawn(server);
//!     let resp = reqwest::get(&format!("http://{}", addr)).await?;
//!     assert_eq!(StatusCode::OK, resp.status());
//!     Ok(())
//! }
//! ```

use std::pin::Pin;
use std::time::Instant;
use std::{io, mem};

use bytes::Bytes;
use bytesize::ByteSize;
use futures::task::{self, Poll};
use futures::{Future, Stream};
use roa_core::http::{Method, StatusCode};
use tracing::{error, info};

use crate::http::Uri;
use crate::{Context, Executor, JoinHandle, Next, Result};

/// A finite-state machine to log success information in each successful response.
enum StreamLogger<S> {
    /// Polling state, as a body stream.
    Polling { stream: S, task: LogTask },

    /// Logging state, as a logger future.
    Logging(JoinHandle<()>),

    /// Complete, as a empty stream.
    Complete,
}

/// A task structure to log when polling is complete.
#[derive(Clone)]
struct LogTask {
    counter: u64,
    method: Method,
    status_code: StatusCode,
    uri: Uri,
    start: Instant,
    exec: Executor,
}

impl LogTask {
    #[inline]
    fn log(&self) -> JoinHandle<()> {
        let LogTask {
            counter,
            method,
            status_code,
            uri,
            start,
            exec,
        } = self.clone();
        exec.spawn_blocking(move || {
            info!(
                "<-- {} {} {}ms {} {}",
                method,
                uri,
                start.elapsed().as_millis(),
                ByteSize(counter),
                status_code,
            )
        })
    }
}

impl<S> Stream for StreamLogger<S>
where
    S: 'static + Send + Send + Unpin + Stream<Item = io::Result<Bytes>>,
{
    type Item = io::Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        match &mut *self {
            StreamLogger::Polling { stream, task } => {
                match futures::ready!(Pin::new(stream).poll_next(cx)) {
                    Some(Ok(bytes)) => {
                        task.counter += bytes.len() as u64;
                        Poll::Ready(Some(Ok(bytes)))
                    }
                    None => {
                        let handler = task.log();
                        *self = StreamLogger::Logging(handler);
                        self.poll_next(cx)
                    }
                    err => Poll::Ready(err),
                }
            }

            StreamLogger::Logging(handler) => {
                futures::ready!(Pin::new(handler).poll(cx));
                *self = StreamLogger::Complete;
                self.poll_next(cx)
            }

            StreamLogger::Complete => Poll::Ready(None),
        }
    }
}

/// A middleware to log information about request and response.
///
/// Based on crate `log`, the log level must be greater than `INFO` to log all information,
/// and should be greater than `ERROR` when you need error information only.
pub async fn logger<S>(ctx: &mut Context<S>, next: Next<'_>) -> Result {
    info!("--> {} {}", ctx.method(), ctx.uri().path());
    let start = Instant::now();
    let mut result = next.await;

    let method = ctx.method().clone();
    let uri = ctx.uri().clone();
    let exec = ctx.exec.clone();

    match &mut result {
        Err(status) => {
            let status_code = status.status_code;
            let message = if status.expose {
                status.message.clone()
            } else {
                // set expose to true; then root status_handler won't log this status.
                status.expose = true;

                // take unexposed message
                mem::take(&mut status.message)
            };
            ctx.exec
                .spawn_blocking(move || {
                    error!("<-- {} {} {}\n{}", method, uri, status_code, message,);
                })
                .await
        }
        Ok(_) => {
            let status_code = ctx.status();
            // logging when body polling complete.
            let logger = StreamLogger::Polling {
                stream: mem::take(&mut ctx.resp.body),
                task: LogTask {
                    counter: 0,
                    method,
                    uri,
                    status_code,
                    start,
                    exec,
                },
            };
            ctx.resp.write_stream(logger);
        }
    }
    result
}
