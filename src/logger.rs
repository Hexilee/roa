use crate::core::{Body, BodyCallback, Context, Model, Next, Result};
use bytesize::ByteSize;
use log::{error, info};
use std::time::Instant;

pub async fn logger<M: Model>(ctx: Context<M>, next: Next) -> Result {
    let start = Instant::now();
    let method = ctx.method().await;
    let uri = ctx.uri().await;
    info!("--> {} {}", method, uri.path());
    let path = uri.path().to_string();
    let result = next().await;
    let callback: Box<BodyCallback> = match result {
        Ok(()) => {
            let status_code = ctx.status().await;
            Box::new(move |body: &Body| {
                info!(
                    "<-- {} {} {}ms {} {}",
                    method,
                    path,
                    start.elapsed().as_millis(),
                    ByteSize(body.consumed() as u64),
                    status_code,
                )
            })
        }
        Err(ref status) => {
            let message = status.message.clone();
            let status_code = status.status_code;
            Box::new(move |_| {
                error!(
                    "<-- {} {} {}ms {}\n{}",
                    method,
                    path,
                    start.elapsed().as_millis(),
                    status_code,
                    message,
                )
            })
        }
    };
    ctx.resp_mut().await.on_finish(callback);
    result
}

#[cfg(test)]
mod tests {
    use super::logger;
    use crate::core::{throw, App};
    use async_std::task::spawn;
    use http::StatusCode;
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
            .gate(logger)
            .gate(move |ctx, _next| async move {
                ctx.resp_mut().await.write_str("Hello, World.");
                Ok(())
            })
            .run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());

        let records = LOGGER.records.read().unwrap().clone();
        assert_eq!(2, records.len());
        assert_eq!("INFO", records[0].0);
        assert_eq!("--> GET /", records[0].1);
        assert_eq!("INFO", records[1].0);
        assert!(records[1].1.starts_with("<-- GET /"));
        assert!(records[1].1.ends_with("13 B"));

        // error
        let (addr, server) = App::new(())
            .gate(logger)
            .gate(move |_ctx, _next| async move { throw(StatusCode::BAD_REQUEST, "Hello, World.") })
            .run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::BAD_REQUEST, resp.status());
        let records = LOGGER.records.read().unwrap().clone();
        assert_eq!(4, records.len());
        assert_eq!("INFO", records[2].0);
        assert_eq!("--> GET /", records[2].1);
        assert_eq!("ERROR", records[3].0);
        assert!(records[3].1.starts_with("<-- GET /"));
        assert!(records[3].1.ends_with("Hello, World."));
        Ok(())
    }
}
