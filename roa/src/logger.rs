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
//!     let mut app = App::new(());
//!     app.gate(logger);
//!     app.end(|mut ctx| async move {
//!         ctx.write_text("Hello, World!")
//!     });
//!     let (addr, server) = app.run()?;
//!     spawn(server);
//!     let resp = reqwest::get(&format!("http://{}", addr)).await?;
//!     assert_eq!(StatusCode::OK, resp.status());
//!     Ok(())
//! }
//! ```

use crate::{Body, Context, Executor, JoinHandle, Next, Result, State};
use bytes::Bytes;
use bytesize::ByteSize;
use futures::task::{self, Poll};
use futures::{Future, Stream};
use log::{error, info};
use roa_core::http::{Method, StatusCode};
use std::io;
use std::pin::Pin;
use std::time::Instant;

/// A finite-state machine to log success information in each streaming response.
enum StreamLogger<S> {
    Polling { stream: S, task: Box<LogTask> },

    Logging(JoinHandle<()>),

    Complete,
}

/// A task structure to log when polling is complete.
#[derive(Clone)]
struct LogTask {
    counter: u64,
    method: Method,
    status_code: StatusCode,
    path: String,
    start: Instant,
    exec: Executor,
}

impl LogTask {
    fn log(self) -> JoinHandle<()> {
        let LogTask {
            counter,
            method,
            status_code,
            path,
            start,
            exec,
        } = self;
        exec.spawn_blocking(move || {
            info!(
                "<-- {} {} {}ms {} {}",
                method,
                path,
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

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        match &mut *self {
            StreamLogger::Polling { stream, task } => {
                match futures::ready!(Pin::new(stream).poll_next(cx)) {
                    Some(Ok(bytes)) => {
                        task.counter += bytes.len() as u64;
                        Poll::Ready(Some(Ok(bytes)))
                    }
                    None => {
                        let handler = task.clone().log();
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
pub async fn logger<S: State>(mut ctx: Context<S>, next: Next) -> Result {
    info!("--> {} {}", ctx.method(), ctx.uri().path());
    let start = Instant::now();
    let result = next.await;

    let method = ctx.method().clone();
    let path = ctx.uri().path().to_string();
    let exec = ctx.exec.clone();
    let status_code = ctx.status();

    match (&result, &mut ctx.resp_mut().body) {
        (Err(err), _) => {
            let message = err.message.clone();
            ctx.exec
                .spawn_blocking(move || {
                    error!(
                        "<-- {} {} {}ms {}\n{}",
                        method,
                        path,
                        start.elapsed().as_millis(),
                        status_code,
                        message,
                    );
                })
                .await
        }
        (Ok(_), Body::Bytes(bytes)) => {
            let size = bytes.size_hint();
            ctx.exec
                .spawn_blocking(move || {
                    info!(
                        "<-- {} {} {}ms {} {}",
                        method,
                        path,
                        start.elapsed().as_millis(),
                        ByteSize(size as u64),
                        status_code,
                    );
                })
                .await
        }
        (Ok(_), Body::Stream(stream)) => {
            let task = Box::new(LogTask {
                counter: 0,
                method,
                path,
                status_code,
                start,
                exec,
            });
            let logger = StreamLogger::Polling {
                stream: std::mem::take(stream),
                task,
            };
            ctx.resp_mut().write_stream(logger);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::logger;
    use crate::http::StatusCode;
    use crate::preload::*;
    use crate::{throw, App};
    use async_std::fs::File;
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

        // bytes info
        let mut app = App::new(());
        app.gate_fn(logger).end(move |mut ctx| async move {
            ctx.resp_mut().write("Hello, World.");
            Ok(())
        });
        let (addr, server) = app.run()?;
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
        app = App::new(());
        app.gate_fn(logger).end(move |_ctx| async move {
            throw!(StatusCode::BAD_REQUEST, "Hello, World!")
        });
        let (addr, server) = app.run()?;
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

        // stream info
        app = App::new(());
        app.gate_fn(logger).end(move |mut ctx| async move {
            ctx.resp_mut()
                .write_reader(File::open("../assets/welcome.html").await?);
            Ok(())
        });
        let (addr, server) = app.run()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        assert_eq!(236, resp.text().await?.len());
        let records = LOGGER.records.read().unwrap().clone();
        assert_eq!(6, records.len());
        assert_eq!("INFO", records[4].0);
        assert_eq!("--> GET /", records[4].1);
        assert_eq!("INFO", records[5].0);
        assert!(records[5].1.starts_with("<-- GET /"));
        assert!(records[5].1.contains("236 B"));
        assert!(records[5].1.ends_with("200 OK"));
        Ok(())
    }
}
