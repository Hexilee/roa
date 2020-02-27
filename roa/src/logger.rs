//! The logger module of roa.
//! This module provides a middleware `logger`.
//!
//! ### Example
//!
//! ```rust
//! use roa::logger::logger;
//! use roa::preload::*;
//! use roa::App;
//! use roa::http::StatusCode;
//! use async_std::task::spawn;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     pretty_env_logger::init();
//!     let (addr, server) = App::new(())
//!         .gate(logger)
//!         .end(|mut ctx| async move {
//!             ctx.write_text("Hello, World!")
//!         })
//!         .run_local()?;
//!     spawn(server);
//!     let resp = reqwest::get(&format!("http://{}", addr)).await?;
//!     assert_eq!(StatusCode::OK, resp.status());
//!     Ok(())
//! }
//! ```

use crate::{Context, Executor, JoinHandle, Next, Result, State};
use bytes::Bytes;
use bytesize::ByteSize;
use futures::task::{self, Poll};
use futures::{Future, Stream};
use log::{error, info};
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

enum Logger<S, F>
where
    F: FnOnce(u64),
{
    Polling {
        counter: u64,
        stream: S,
        exec: Executor,
        task: Arc<F>,
    },

    Logging(JoinHandle<()>),

    Complete,
}

impl<S, F> Stream for Logger<S, F>
where
    F: 'static + Send + Sync + Unpin + Fn(u64),
    S: 'static + Send + Send + Unpin + Stream<Item = io::Result<Bytes>>,
{
    type Item = io::Result<Bytes>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        match &mut *self {
            Logger::Polling {
                stream,
                exec,
                counter,
                task,
            } => match futures::ready!(Pin::new(stream).poll_next(cx)) {
                Some(Ok(bytes)) => {
                    *counter += bytes.len() as u64;
                    Poll::Ready(Some(Ok(bytes)))
                }
                None => {
                    let counter = *counter;
                    let task = task.clone();
                    let handler = exec.spawn_blocking(move || task(counter));
                    *self = Logger::Logging(handler);
                    self.poll_next(cx)
                }
                err => Poll::Ready(err),
            },

            Logger::Logging(handler) => {
                futures::ready!(Pin::new(handler).poll(cx));
                *self = Logger::Complete;
                println!("logger complete");
                self.poll_next(cx)
            }

            Logger::Complete => Poll::Ready(None),
        }
    }
}

/// A middleware to log information about request and response.
///
/// Based on crate `log`, the log level must be greater than `INFO` to log all information,
/// and should be greater than `ERROR` when you need error information only.
pub async fn logger<S: State>(mut ctx: Context<S>, next: Next) -> Result {
    info!("--> {} {}", ctx.method(), ctx.uri().path());
    let start = Instant::now();
    let result = next.await;

    let method = ctx.method().clone();
    let path = ctx.uri().path().to_string();
    let counter = 0;
    let exec = ctx.exec().clone();
    match result {
        Ok(()) => {
            let status_code = ctx.status();
            ctx.resp_mut().map_body(move |stream| Logger::Polling {
                counter,
                stream,
                exec,
                task: Arc::new(move |counter| {
                    info!(
                        "<-- {} {} {}ms {} {}",
                        method,
                        path,
                        start.elapsed().as_millis(),
                        ByteSize(counter),
                        status_code,
                    );
                }),
            });
        }
        Err(ref err) => {
            let message = err.message.clone();
            let status_code = err.status_code;
            ctx.resp_mut().map_body(move |stream| Logger::Polling {
                counter,
                stream,
                exec,
                task: Arc::new(move |_counter| {
                    error!(
                        "<-- {} {} {}ms {}\n{}",
                        method,
                        path,
                        start.elapsed().as_millis(),
                        status_code,
                        message,
                    );
                }),
            });
        }
    };
    result
}

#[cfg(test)]
mod tests {
    use super::logger;
    use crate::http::StatusCode;
    use crate::preload::*;
    use crate::{throw, App};
    use async_std::task::spawn;
    use lazy_static::lazy_static;
    use log::{Level, LevelFilter, Metadata, Record, SetLoggerError};
    use std::sync::RwLock;

    struct TestLogger {
        records: RwLock<Vec<(String, String)>>,
    }
    impl log::Log for TestLogger {
        fn enabled(&self, metadata: &Metadata) -> bool {
            metadata.level() <= Level::Info
        }
        fn log(&self, record: &Record) {
            self.records
                .write()
                .unwrap()
                .push((record.level().to_string(), record.args().to_string()))
        }
        fn flush(&self) {}
    }

    lazy_static! {
        static ref LOGGER: TestLogger = TestLogger {
            records: RwLock::new(Vec::new()),
        };
    }

    fn init() -> Result<(), SetLoggerError> {
        log::set_logger(&*LOGGER).map(|()| log::set_max_level(LevelFilter::Info))
    }

    #[tokio::test]
    async fn log() -> Result<(), Box<dyn std::error::Error>> {
        init()?;

        // info
        let (addr, server) = App::new(())
            .gate_fn(logger)
            .end(move |mut ctx| async move {
                ctx.resp_mut().write_str("Hello, World.");
                Ok(())
            })
            .run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        assert_eq!("Hello, World.", resp.text().await?);
        let records = LOGGER.records.read().unwrap().clone();
        assert_eq!(2, records.len());
        assert_eq!("INFO", records[0].0);
        assert_eq!("--> GET /", records[0].1);
        assert_eq!("INFO", records[1].0);
        assert!(records[1].1.starts_with("<-- GET /"));
        assert!(records[1].1.contains("13 B"));
        assert!(records[1].1.ends_with("200 OK"));

        // error
        let (addr, server) = App::new(())
            .gate_fn(logger)
            .gate_fn(move |_ctx, _next| async move {
                throw!(StatusCode::BAD_REQUEST, "Hello, World!")
            })
            .run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::BAD_REQUEST, resp.status());
        assert_eq!("Hello, World!", resp.text().await?);
        let records = LOGGER.records.read().unwrap().clone();
        assert_eq!(4, records.len());
        assert_eq!("INFO", records[2].0);
        assert_eq!("--> GET /", records[2].1);
        assert_eq!("ERROR", records[3].0);
        assert!(records[3].1.starts_with("<-- GET /"));
        assert!(records[3].1.ends_with("Hello, World!"));
        Ok(())
    }
}
