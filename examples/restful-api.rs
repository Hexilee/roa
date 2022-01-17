//! RUST_LOG=info Cargo run --example restful-api,
//! then:
//! - `curl 127.0.0.1:8000/user/0`
//!     query user where id=0
//! - `curl -H "Content-type: application/json" -d '{"name":"Hexilee", "age": 20}' -X POST 127.0.0.1:8000/user`
//!     create a new user
//! - `curl -H "Content-type: application/json" -d '{"name":"Alice", "age": 20}' -X PUT 127.0.0.1:8000/user/0`
//!     update user where id=0, return the old data
//! - `curl 127.0.0.1:8000/user/0 -X DELETE`
//!     delete user where id=0

use std::result::Result as StdResult;
use std::sync::Arc;

use roa::http::StatusCode;
use roa::preload::*;
use roa::router::{get, post, Router};
use roa::{throw, App, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use slab::Slab;
use tokio::sync::RwLock;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct User {
    name: String,
    age: u8,
}

#[derive(Clone)]
struct Database {
    table: Arc<RwLock<Slab<User>>>,
}

impl Database {
    fn new() -> Self {
        Self {
            table: Arc::new(RwLock::new(Slab::new())),
        }
    }

    async fn create(&self, user: User) -> usize {
        self.table.write().await.insert(user)
    }

    async fn retrieve(&self, id: usize) -> Result<User> {
        match self.table.read().await.get(id) {
            Some(user) => Ok(user.clone()),
            None => throw!(StatusCode::NOT_FOUND),
        }
    }

    async fn update(&self, id: usize, new_user: &mut User) -> Result {
        match self.table.write().await.get_mut(id) {
            Some(user) => {
                std::mem::swap(new_user, user);
                Ok(())
            }
            None => throw!(StatusCode::NOT_FOUND),
        }
    }

    async fn delete(&self, id: usize) -> Result<User> {
        if !self.table.read().await.contains(id) {
            throw!(StatusCode::NOT_FOUND)
        }
        Ok(self.table.write().await.remove(id))
    }
}

async fn create_user(ctx: &mut Context<Database>) -> Result {
    let user: User = ctx.read_json().await?;
    let id = ctx.create(user).await;
    ctx.write_json(&json!({ "id": id }))?;
    ctx.resp.status = StatusCode::CREATED;
    Ok(())
}

async fn get_user(ctx: &mut Context<Database>) -> Result {
    let id: usize = ctx.must_param("id")?.parse()?;
    let user = ctx.retrieve(id).await?;
    ctx.write_json(&user)
}

async fn update_user(ctx: &mut Context<Database>) -> Result {
    let id: usize = ctx.must_param("id")?.parse()?;
    let mut user: User = ctx.read_json().await?;
    ctx.update(id, &mut user).await?;
    ctx.write_json(&user)
}

async fn delete_user(ctx: &mut Context<Database>) -> Result {
    let id: usize = ctx.must_param("id")?.parse()?;
    let user = ctx.delete(id).await?;
    ctx.write_json(&user)
}

#[tokio::main]
async fn main() -> StdResult<(), Box<dyn std::error::Error>> {
    let router = Router::new()
        .on("/", post(create_user))
        .on("/:id", get(get_user).put(update_user).delete(delete_user));
    let app = App::state(Database::new()).end(router.routes("/user")?);
    app.listen("127.0.0.1:8000", |addr| {
        println!("Server is listening on {}", addr)
    })?
    .await?;
    Ok(())
}
