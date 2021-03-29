//! This module provides a context extension `Forward`,
//! which is used to parse `X-Forwarded-*` headers.

use std::net::IpAddr;

use crate::http::header::HOST;
use crate::{Context, State};

/// A context extension `Forward` used to parse `X-Forwarded-*` request headers.
pub trait Forward {
    /// Get true host.
    /// - If "x-forwarded-host" is set and valid, use it.
    /// - Else if "host" is set and valid, use it.
    /// - Else throw Err(400 BAD REQUEST).
    ///
    /// ### Example
    /// ```rust
    /// use roa::{Context, Result};
    /// use roa::forward::Forward;
    ///
    /// async fn get(ctx: &mut Context) -> Result {
    ///     if let Some(host) = ctx.host() {
    ///         println!("host: {}", host);
    ///     }
    ///     Ok(())
    /// }
    /// ```
    fn host(&self) -> Option<&str>;

    /// Get true client ip.
    /// - If "x-forwarded-for" is set and valid, use the first ip.
    /// - Else use the ip of `Context::remote_addr()`.
    ///
    /// ### Example
    /// ```rust
    /// use roa::{Context, Result};
    /// use roa::forward::Forward;
    ///
    /// async fn get(ctx: &mut Context) -> Result {
    ///     println!("client ip: {}", ctx.client_ip());
    ///     Ok(())
    /// }
    /// ```
    fn client_ip(&self) -> IpAddr;

    /// Get true forwarded ips.
    /// - If "x-forwarded-for" is set and valid, use it.
    /// - Else return an empty vector.
    ///
    /// ### Example
    /// ```rust
    /// use roa::{Context, Result};
    /// use roa::forward::Forward;
    ///
    /// async fn get(ctx: &mut Context) -> Result {
    ///     println!("forwarded ips: {:?}", ctx.forwarded_ips());
    ///     Ok(())
    /// }
    /// ```
    fn forwarded_ips(&self) -> Vec<IpAddr>;

    /// Try to get forwarded proto.
    /// - If "x-forwarded-proto" is not set, return None.
    /// - If "x-forwarded-proto" is set but fails to string, return Some(Err(400 BAD REQUEST)).
    ///
    /// ### Example
    /// ```rust
    /// use roa::{Context, Result};
    /// use roa::forward::Forward;
    ///
    /// async fn get(ctx: &mut Context) -> Result {
    ///     if let Some(proto) = ctx.forwarded_proto() {
    ///         println!("forwarded proto: {}", proto);
    ///     }
    ///     Ok(())
    /// }
    /// ```
    fn forwarded_proto(&self) -> Option<&str>;
}

impl<S: State> Forward for Context<S> {
    #[inline]
    fn host(&self) -> Option<&str> {
        self.get("x-forwarded-host").or_else(|| self.get(HOST))
    }

    #[inline]
    fn client_ip(&self) -> IpAddr {
        let addrs = self.forwarded_ips();
        if addrs.is_empty() {
            self.remote_addr.ip()
        } else {
            addrs[0]
        }
    }

    #[inline]
    fn forwarded_ips(&self) -> Vec<IpAddr> {
        let mut addrs = Vec::new();
        if let Some(value) = self.get("x-forwarded-for") {
            for addr_str in value.split(',') {
                if let Ok(addr) = addr_str.trim().parse() {
                    addrs.push(addr)
                }
            }
        }
        addrs
    }

    #[inline]
    fn forwarded_proto(&self) -> Option<&str> {
        self.get("x-forwarded-proto")
    }
}

#[cfg(all(test, feature = "tcp"))]
mod tests {
    use async_std::task::spawn;

    use super::Forward;
    use crate::http::header::HOST;
    use crate::http::{HeaderValue, StatusCode};
    use crate::preload::*;
    use crate::{App, Context};

    #[tokio::test]
    async fn host() -> Result<(), Box<dyn std::error::Error>> {
        async fn test(ctx: &mut Context) -> crate::Result {
            assert_eq!(Some("github.com"), ctx.host());
            Ok(())
        }
        let (addr, server) = App::new().end(test).run()?;
        spawn(server);
        let client = reqwest::Client::new();
        let resp = client
            .get(&format!("http://{}", addr))
            .header(HOST, HeaderValue::from_static("github.com"))
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());

        let resp = client
            .get(&format!("http://{}", addr))
            .header(HOST, "google.com")
            .header("x-forwarded-host", "github.com")
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn host_err() -> Result<(), Box<dyn std::error::Error>> {
        async fn test(ctx: &mut Context) -> crate::Result {
            ctx.req.headers.remove(HOST);
            assert_eq!(None, ctx.host());
            Ok(())
        }
        let (addr, server) = App::new().end(test).run()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn client_ip() -> Result<(), Box<dyn std::error::Error>> {
        async fn remote_addr(ctx: &mut Context) -> crate::Result {
            assert_eq!(ctx.remote_addr.ip(), ctx.client_ip());
            Ok(())
        }
        let (addr, server) = App::new().end(remote_addr).run()?;
        spawn(server);
        reqwest::get(&format!("http://{}", addr)).await?;

        async fn forward_addr(ctx: &mut Context) -> crate::Result {
            assert_eq!("192.168.0.1", ctx.client_ip().to_string());
            Ok(())
        }
        let (addr, server) = App::new().end(forward_addr).run()?;
        spawn(server);
        let client = reqwest::Client::new();
        client
            .get(&format!("http://{}", addr))
            .header("x-forwarded-for", "192.168.0.1, 8.8.8.8")
            .send()
            .await?;

        Ok(())
    }

    #[tokio::test]
    async fn forwarded_proto() -> Result<(), Box<dyn std::error::Error>> {
        async fn test(ctx: &mut Context) -> crate::Result {
            assert_eq!(Some("https"), ctx.forwarded_proto());
            Ok(())
        }
        let (addr, server) = App::new().end(test).run()?;
        spawn(server);
        let client = reqwest::Client::new();
        client
            .get(&format!("http://{}", addr))
            .header("x-forwarded-proto", "https")
            .send()
            .await?;

        Ok(())
    }
}
