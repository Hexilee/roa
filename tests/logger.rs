use std::sync::RwLock;

use async_std::fs::File;
use async_std::task::spawn;
use log::{Level, LevelFilter, Metadata, Record};
use once_cell::sync::Lazy;
use roa::http::StatusCode;
use roa::logger::logger;
use roa::preload::*;
use roa::{throw, App, Context};

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

static LOGGER: Lazy<TestLogger> = Lazy::new(|| TestLogger {
    records: RwLock::new(Vec::new()),
});

fn init() -> anyhow::Result<()> {
    log::set_logger(&*LOGGER)
        .map(|()| log::set_max_level(LevelFilter::Info))
        .map_err(|err| anyhow::anyhow!("fail to init logger: {}", err))
}

#[tokio::test]
async fn log() -> anyhow::Result<()> {
    init()?;
    async fn bytes_info(ctx: &mut Context) -> roa::Result {
        ctx.resp.write("Hello, World.");
        Ok(())
    }
    // bytes info
    let (addr, server) = App::new().gate(logger).end(bytes_info).run()?;
    spawn(server);
    let resp = reqwest::get(&format!("http://{}", addr)).await?;
    assert_eq!(StatusCode::OK, resp.status());
    assert_eq!("Hello, World.", resp.text().await?);
    let records = LOGGER.records.read().unwrap().clone();
    assert_eq!(2, records.len());
    assert_eq!("INFO", records[0].0);
    assert_eq!("--> GET /", records[0].1.trim_end());
    assert_eq!("INFO", records[1].0);
    assert!(records[1].1.starts_with("<-- GET /"));
    assert!(records[1].1.contains("13 B"));
    assert!(records[1].1.trim_end().ends_with("200 OK"));

    // error
    async fn err(_ctx: &mut Context) -> roa::Result {
        throw!(StatusCode::BAD_REQUEST, "Hello, World!")
    }
    let (addr, server) = App::new().gate(logger).end(err).run()?;
    spawn(server);
    let resp = reqwest::get(&format!("http://{}", addr)).await?;
    assert_eq!(StatusCode::BAD_REQUEST, resp.status());
    assert_eq!("Hello, World!", resp.text().await?);
    let records = LOGGER.records.read().unwrap().clone();
    assert_eq!(4, records.len());
    assert_eq!("INFO", records[2].0);
    assert_eq!("--> GET /", records[2].1.trim_end());
    assert_eq!("ERROR", records[3].0);
    assert!(records[3].1.starts_with("<-- GET /"));
    assert!(records[3].1.contains(&StatusCode::BAD_REQUEST.to_string()));
    assert!(records[3].1.trim_end().ends_with("Hello, World!"));

    // stream info
    async fn stream_info(ctx: &mut Context) -> roa::Result {
        ctx.resp
            .write_reader(File::open("assets/welcome.html").await?);
        Ok(())
    }
    // bytes info
    let (addr, server) = App::new().gate(logger).end(stream_info).run()?;
    spawn(server);
    let resp = reqwest::get(&format!("http://{}", addr)).await?;
    assert_eq!(StatusCode::OK, resp.status());
    assert_eq!(236, resp.text().await?.len());
    let records = LOGGER.records.read().unwrap().clone();
    assert_eq!(6, records.len());
    assert_eq!("INFO", records[4].0);
    assert_eq!("--> GET /", records[4].1.trim_end());
    assert_eq!("INFO", records[5].0);
    assert!(records[5].1.starts_with("<-- GET /"));
    assert!(records[5].1.contains("236 B"));
    assert!(records[5].1.trim_end().ends_with("200 OK"));
    Ok(())
}
