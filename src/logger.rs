use crate::{Body, BodyCallback, Context, Model, Next, Status};
use bytesize::ByteSize;
use log::{error, info};
use std::time::Instant;

pub async fn logger<M: Model>(ctx: Context<M>, next: Next) -> Result<(), Status> {
    let start = Instant::now();
    let method = ctx.method().await;
    let uri = ctx.uri().await;
    info!("--> {} {}", method, uri.path());
    let path = uri.path().to_string();
    let result = next().await;
    let status = match result {
        Ok(()) => ctx.status().await,
        Err(ref status) => status.status_code,
    };
    let callback: Box<BodyCallback> = match status.as_u16() / 100 {
        4 | 5 => Box::new(move |body: &Body| {
            error!(
                "<-- {} {} {}ms {}",
                method,
                path,
                start.elapsed().as_millis(),
                ByteSize(body.consumed() as u64)
            )
        }),
        _ => Box::new(move |body: &Body| {
            info!(
                "<-- {} {} {}ms {}",
                method,
                path,
                start.elapsed().as_millis(),
                ByteSize(body.consumed() as u64)
            )
        }),
    };
    ctx.resp_mut().await.on_finish(callback);
    result
}

#[cfg(test)]
mod tests {
    use super::logger;
    use crate::{App, Request};
    use futures::AsyncReadExt;
    use http::StatusCode;
    use lazy_static::lazy_static;
    use log::{Level, LevelFilter, Metadata, Record, SetLoggerError};
    use roa_core::throw;
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
        let mut resp = App::new(())
            .gate(logger)
            .gate(move |ctx, _next| async move {
                ctx.resp_mut().await.write_str("Hello, World.");
                Ok(())
            })
            .serve(Request::new(), "127.0.0.1:8000".parse()?)
            .await?;
        let (level, data) = LOGGER.records.write().unwrap().pop().unwrap();
        assert_eq!("INFO", level);
        assert_eq!("--> GET /", data);
        resp.read_to_string(&mut String::new()).await?;
        let (level, data) = LOGGER.records.write().unwrap().pop().unwrap();
        assert_eq!("INFO", level);
        assert_eq!("<-- GET / 0ms 13 B", data);

        // error
        resp = App::new(())
            .gate(logger)
            .gate(move |_ctx, _next| async move { throw(StatusCode::BAD_REQUEST, "Hello, World.") })
            .serve(Request::new(), "127.0.0.1:8000".parse()?)
            .await?;
        let (level, data) = LOGGER.records.write().unwrap().pop().unwrap();
        assert_eq!("INFO", level);
        assert_eq!("--> GET /", data);
        resp.read_to_string(&mut String::new()).await?;
        let (level, data) = LOGGER.records.write().unwrap().pop().unwrap();
        assert_eq!("ERROR", level);
        assert_eq!("<-- GET / 0ms 13 B", data);
        Ok(())
    }
}
