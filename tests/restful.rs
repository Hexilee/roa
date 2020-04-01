use async_std::sync::{Arc, RwLock};
use async_std::task::spawn;
use http::StatusCode;
use multimap::MultiMap;
use roa::preload::*;
use roa::query::query_parser;
use roa::router::{get, post, Router};
use roa::{throw, App, Context};
use serde::{Deserialize, Serialize};
use serde_json::json;
use slab::Slab;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize, Hash, Eq, PartialEq)]
struct User {
    name: String,
    age: u8,
    favorite_fruit: String,
}

struct DB {
    main_table: Slab<User>,
    name_index: MultiMap<String, usize>,
}

impl DB {
    fn new() -> Self {
        Self {
            main_table: Slab::new(),
            name_index: MultiMap::new(),
        }
    }

    fn add(&mut self, user: User) -> usize {
        let name = user.name.clone();
        let id = self.main_table.insert(user);
        self.name_index.insert(name, id);
        id
    }

    fn delete_index(&mut self, name: &str, id: usize) {
        if let Some(id_set) = self.name_index.get_vec_mut(name) {
            let uids = id_set.clone();
            for (index, uid) in uids.into_iter().enumerate() {
                if uid == id {
                    id_set.remove(index);
                }
            }
        }
    }

    fn delete(&mut self, id: usize) -> Option<User> {
        if !self.main_table.contains(id) {
            None
        } else {
            let user = self.main_table.remove(id);
            self.delete_index(&user.name, id);
            Some(user)
        }
    }

    fn get(&self, id: usize) -> Option<&User> {
        self.main_table.get(id)
    }

    fn get_by_name(&self, name: &str) -> Vec<(usize, &User)> {
        match self.name_index.get_vec(name) {
            None => Vec::new(),
            Some(ids) => ids
                .iter()
                .filter_map(|id| self.get(*id).map(|user| (*id, user)))
                .collect(),
        }
    }

    fn update(&mut self, id: usize, new_user: &mut User) -> bool {
        let new_name = new_user.name.clone();
        let swapped = self
            .main_table
            .get_mut(id)
            .map(|user| std::mem::swap(user, new_user))
            .is_some();
        if swapped {
            self.delete_index(&new_user.name, id);
            self.name_index.insert(new_name, id);
        }
        swapped
    }
}

#[derive(Clone)]
struct State(Arc<RwLock<DB>>);

impl State {
    fn new(db: DB) -> Self {
        Self(Arc::new(RwLock::new(db)))
    }

    async fn add(&mut self, user: User) -> usize {
        self.0.write().await.add(user)
    }

    async fn delete(&mut self, id: usize) -> Option<User> {
        self.0.write().await.delete(id)
    }

    async fn get_user(&self, id: usize) -> Option<User> {
        self.0.read().await.get(id).cloned()
    }

    async fn get_by_name(&self, name: &str) -> Vec<(usize, User)> {
        self.0
            .read()
            .await
            .get_by_name(name)
            .into_iter()
            .map(|(id, user)| (id, user.clone()))
            .collect()
    }

    async fn get_all(&self) -> Vec<(usize, User)> {
        self.0
            .read()
            .await
            .main_table
            .iter()
            .map(|(id, user)| (id, user.clone()))
            .collect()
    }

    async fn update(&mut self, id: usize, new_user: &mut User) -> bool {
        self.0.write().await.update(id, new_user)
    }
}

async fn create_user(ctx: &mut Context<State>) -> roa::Result {
    let user = ctx.read_json().await?;
    let id = ctx.add(user).await;
    ctx.resp.status = StatusCode::CREATED;
    ctx.write_json(&json!({ "id": id }))
}

async fn query_user(ctx: &mut Context<State>) -> roa::Result {
    let id = ctx.must_param("id")?.parse()?;
    match ctx.get_user(id).await {
        Some(user) => ctx.write_json(&user),
        None => throw!(StatusCode::NOT_FOUND, format!("id({}) not found", id)),
    }
}

async fn update_user(ctx: &mut Context<State>) -> roa::Result {
    let id = ctx.must_param("id")?.parse()?;
    let mut user = ctx.read_json().await?;
    if ctx.update(id, &mut user).await {
        ctx.write_json(&user)
    } else {
        throw!(StatusCode::NOT_FOUND, format!("id({}) not found", id))
    }
}

async fn delete_user(ctx: &mut Context<State>) -> roa::Result {
    let id = ctx.must_param("id")?.parse()?;
    match ctx.delete(id).await {
        Some(user) => ctx.write_json(&user),
        None => throw!(StatusCode::NOT_FOUND, format!("id({}) not found", id)),
    }
}

fn crud_router() -> Router<State> {
    Router::new()
        .on("/", post(create_user))
        .on("/:id", get(query_user).put(update_user).delete(delete_user))
}

#[tokio::test]
async fn restful_crud() -> Result<(), Box<dyn std::error::Error>> {
    let app = App::state(State::new(DB::new())).end(crud_router().routes("/user")?);
    let (addr, server) = app.run()?;
    spawn(server);
    // first get, 404 Not Found
    let resp = reqwest::get(&format!("http://{}/user/0", addr)).await?;
    assert_eq!(StatusCode::NOT_FOUND, resp.status());

    let user = User {
        name: "Hexilee".to_string(),
        age: 20,
        favorite_fruit: "Apple".to_string(),
    };

    // post
    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("http://{}/user", addr))
        .json(&user)
        .send()
        .await?;
    assert_eq!(StatusCode::CREATED, resp.status());
    let data: HashMap<String, usize> = serde_json::from_str(&resp.text().await?)?;
    assert_eq!(0, data["id"]);

    // get
    let resp = reqwest::get(&format!("http://{}/user/0", addr)).await?;
    assert_eq!(StatusCode::OK, resp.status());
    assert_eq!(&user, &resp.json().await?);

    // put
    let another = User {
        name: "Bob".to_string(),
        age: 120,
        favorite_fruit: "Lemon".to_string(),
    };

    let resp = client
        .put(&format!("http://{}/user/0", addr))
        .json(&another)
        .send()
        .await?;
    assert_eq!(StatusCode::OK, resp.status());

    // return first user
    assert_eq!(&user, &resp.json().await?);

    // updated, get new user
    let resp = reqwest::get(&format!("http://{}/user/0", addr)).await?;
    assert_eq!(StatusCode::OK, resp.status());
    assert_eq!(&another, &resp.json().await?);

    // delete, get deleted user
    let resp = client
        .delete(&format!("http://{}/user/0", addr))
        .send()
        .await?;
    assert_eq!(StatusCode::OK, resp.status());
    assert_eq!(&another, &resp.json().await?);

    // delete again, 404 Not Found
    let resp = client
        .delete(&format!("http://{}/user/0", addr))
        .send()
        .await?;
    assert_eq!(StatusCode::NOT_FOUND, resp.status());

    // put again, 404 Not Found
    let resp = client
        .put(&format!("http://{}/user/0", addr))
        .json(&another)
        .send()
        .await?;
    assert_eq!(StatusCode::NOT_FOUND, resp.status());
    Ok(())
}

async fn create_batch(ctx: &mut Context<State>) -> roa::Result {
    let users: Vec<User> = ctx.read_json().await?;
    let mut ids = Vec::new();
    for user in users {
        ids.push(ctx.add(user).await)
    }
    ctx.resp.status = StatusCode::CREATED;
    ctx.write_json(&ids)
}

async fn query_batch(ctx: &mut Context<State>) -> roa::Result {
    let users = match ctx.query("name") {
        Some(name) => ctx.get_by_name(&name).await,
        None => ctx.get_all().await,
    };
    ctx.write_json(&users)
}

fn batch_router() -> Router<State> {
    Router::new().on("/user", get(query_batch).post(create_batch))
}

#[tokio::test]
async fn batch() -> Result<(), Box<dyn std::error::Error>> {
    let app = App::state(State::new(DB::new()))
        .gate(query_parser)
        .end(batch_router().routes("/")?);
    let (addr, server) = app.run()?;
    spawn(server);
    // first get, list empty
    let resp = reqwest::get(&format!("http://{}/user", addr)).await?;
    assert_eq!(StatusCode::OK, resp.status());
    let data: Vec<(usize, User)> = resp.json().await?;
    assert!(data.is_empty());

    // post batch
    let client = reqwest::Client::new();
    let users = vec![
        User {
            name: "Hexilee".to_string(),
            age: 20,
            favorite_fruit: "Apple".to_string(),
        },
        User {
            name: "Bob".to_string(),
            age: 120,
            favorite_fruit: "Lemon".to_string(),
        },
        User {
            name: "Hexilee".to_string(),
            age: 40,
            favorite_fruit: "Orange".to_string(),
        },
    ];
    let resp = client
        .post(&format!("http://{}/user", addr))
        .json(&users)
        .send()
        .await?;
    assert_eq!(StatusCode::CREATED, resp.status());
    let ids: Vec<usize> = resp.json().await?;
    assert_eq!(vec![0, 1, 2], ids);

    // get all
    let resp = reqwest::get(&format!("http://{}/user", addr)).await?;
    assert_eq!(StatusCode::OK, resp.status());
    let data: Vec<(usize, User)> = resp.json().await?;
    assert_eq!(3, data.len());
    for (index, user) in data.iter() {
        assert_eq!(&users[*index], user);
    }

    // get by name
    let resp = reqwest::get(&format!("http://{}/user?name=Alice", addr)).await?;
    assert_eq!(StatusCode::OK, resp.status());
    let data: Vec<(usize, User)> = resp.json().await?;
    assert!(data.is_empty());

    let resp = reqwest::get(&format!("http://{}/user?name=Hexilee", addr)).await?;
    assert_eq!(StatusCode::OK, resp.status());
    let data: Vec<(usize, User)> = resp.json().await?;
    assert_eq!(2, data.len());
    assert_eq!(0, data[0].0);
    assert_eq!(&users[0], &data[0].1);
    assert_eq!(2, data[1].0);
    assert_eq!(&users[2], &data[1].1);
    Ok(())
}
