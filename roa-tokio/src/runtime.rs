use roa::Spawn;
use std::future::Future;
use std::pin::Pin;

/// Future Object
pub type FutureObj = Pin<Box<dyn 'static + Send + Future<Output = ()>>>;

/// Blocking task Object
pub type BlockingObj = Box<dyn 'static + Send + FnOnce()>;

/// Tokio-based executor.
///
/// ```
/// use roa::App;
/// use roa_tokio::Exec;
///
/// let app = App::with_exec((), Exec);
/// ```
pub struct Exec;

impl Spawn for Exec {
    #[inline]
    fn spawn(&self, fut: FutureObj) {
        tokio::spawn(fut);
    }

    #[inline]
    fn spawn_blocking(&self, task: BlockingObj) {
        tokio::task::spawn_blocking(task);
    }
}

#[cfg(test)]
mod tests {
    use super::Exec;
    use roa::http::StatusCode;
    use roa::tcp::Listener;
    use roa::App;
    use std::error::Error;

    #[tokio::test]
    async fn exec() -> Result<(), Box<dyn Error>> {
        let app = App::with_exec((), Exec).end(());
        let (addr, server) = app.bind("127.0.0.1:0")?;
        tokio::spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }
}
