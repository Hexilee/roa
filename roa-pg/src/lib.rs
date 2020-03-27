#![cfg_attr(feature = "docs", feature(external_doc))]
#![cfg_attr(feature = "docs", doc(include = "../README.md"))]
#![cfg_attr(feature = "docs", warn(missing_docs))]

mod tls;
pub use tls::{connect_tls, ClientConfig, TlsStream};

#[doc(inline)]
pub use tokio_postgres::*;

use async_std::net::TcpStream;
use roa::tcp::WrapStream;
use std::io;

/// Connect to postgres server.
///
/// ```rust
/// use roa_pg::connect;
/// use std::error::Error;
/// use async_std::task::spawn;
///
/// async fn play() -> Result<(), Box<dyn Error>> {
///     let url = "host=localhost user=postgres";
///     let (client, conn) = connect(&url.parse()?).await?;
///     spawn(conn);
///     let row = client.query_one("SELECT * FROM user WHERE id=$1", &[&0]).await?;
///     let value: &str = row.get(0);
///     println!("value: {}", value);
///     Ok(())
/// }
/// ```
#[inline]
pub async fn connect(
    config: &Config,
) -> io::Result<(
    Client,
    Connection<WrapStream<TcpStream>, TlsStream<WrapStream<TcpStream>>>,
)> {
    connect_tls(config, ClientConfig::default()).await
}
