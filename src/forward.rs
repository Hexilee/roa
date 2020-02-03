use crate::{throw, Context, Model, Status};
use async_trait::async_trait;
use http::{
    header::{HeaderName, HOST},
    StatusCode,
};
use std::net::SocketAddr;

#[async_trait]
pub trait Forward {
    async fn host(&self) -> Result<String, Status>;
    async fn client_addr(&self) -> SocketAddr;
    async fn forwarded_addrs(&self) -> Vec<SocketAddr>;
    async fn forwarded_proto(&self) -> Option<Result<String, Status>>;
}

#[async_trait]
impl<M: Model> Forward for Context<M> {
    async fn host(&self) -> Result<String, Status> {
        if let Some(Ok(value)) = self
            .header(&HeaderName::from_static("X-Forwarded-Host"))
            .await
        {
            Ok(value)
        } else if let Some(Ok(value)) = self.header(&HOST).await {
            Ok(value)
        } else {
            throw(StatusCode::BAD_REQUEST, "header HOST is not set")
        }
    }

    async fn client_addr(&self) -> SocketAddr {
        let addrs = self.forwarded_addrs().await;
        if addrs.is_empty() {
            self.peer_addr
        } else {
            addrs[0]
        }
    }

    async fn forwarded_addrs(&self) -> Vec<SocketAddr> {
        let mut addrs = Vec::new();
        if let Some(Ok(value)) = self
            .header(&HeaderName::from_static("X-Forwarded-For"))
            .await
        {
            for addr_str in value.split(',') {
                if let Ok(addr) = addr_str.trim().parse() {
                    addrs.push(addr)
                }
            }
        }
        addrs
    }

    async fn forwarded_proto(&self) -> Option<Result<String, Status>> {
        self.header(&HeaderName::from_static("X-Forwarded-Proto"))
            .await
            .map(|value| {
                value.map_err(|err| {
                    Status::new(
                        StatusCode::BAD_REQUEST,
                        format!("{}\nvalue of X-Forwarded-Proto is not a valid string", err),
                        true,
                    )
                })
            })
    }
}
