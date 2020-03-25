#[macro_use]
extern crate diesel;

mod models;
mod schema;
use diesel::prelude::*;
use diesel::result::Error;
use diesel_example::{create_pool, State};
use juniper::{graphql_value, FieldError, FieldResult, GraphQLInputObject, RootNode};
use log::info;
use roa::diesel::{AsyncPool, SqlQuery};
use roa::http::{header, Method};
use roa::logger::logger;
use roa::preload::*;
use roa::router::{allow, get, Router};
use roa::{App, Context};
use roa_juniper::{GraphQL, JuniperContext};
use serde::Serialize;
use std::error::Error as StdError;

use crate::models::Post;
use crate::schema::posts;

#[derive(Debug, Insertable, Serialize, GraphQLInputObject)]
#[table_name = "posts"]
#[graphql(description = "A new post")]
struct NewPost {
    title: String,
    body: String,
    published: bool,
}

struct Query;

#[juniper::graphql_object(
    Context = JuniperContext<State>,
)]
impl Query {
    async fn post(
        &self,
        ctx: &JuniperContext<State>,
        id: i32,
        published: bool,
    ) -> FieldResult<Post> {
        use crate::schema::posts::dsl::{self, posts};
        match ctx
            .first(posts.find(id).filter(dsl::published.eq(published)))
            .await?
        {
            Some(post) => Ok(post),
            None => Err(FieldError::new(
                "post not found",
                graphql_value!({ "status": 404, "id": id }),
            )),
        }
    }
}

struct Mutation;

#[juniper::graphql_object(
    Context = JuniperContext<State>,
)]
impl Mutation {
    async fn create_post(
        &self,
        ctx: &JuniperContext<State>,
        new_post: NewPost,
    ) -> FieldResult<Post> {
        use crate::schema::posts::dsl::{self, posts};
        let conn = ctx.get_conn().await?;
        let post = ctx
            .exec
            .spawn_blocking(move || {
                conn.transaction::<Post, Error, _>(|| {
                    diesel::insert_into(crate::schema::posts::table)
                        .values(&new_post)
                        .execute(&conn)?;
                    Ok(posts.order(dsl::id.desc()).first(&conn)?)
                })
            })
            .await?;
        Ok(post)
    }

    async fn update_post(
        &self,
        id: i32,
        ctx: &JuniperContext<State>,
        new_post: NewPost,
    ) -> FieldResult<Post> {
        use crate::schema::posts::dsl::{self, posts};
        match ctx.first(posts.find(id)).await? {
            None => Err(FieldError::new(
                "post not found",
                graphql_value!({ "status": 404, "id": id }),
            )),
            Some(old_post) => {
                let NewPost {
                    title,
                    body,
                    published,
                } = new_post;
                ctx.execute(diesel::update(posts.find(id)).set((
                    dsl::title.eq(title),
                    dsl::body.eq(body),
                    dsl::published.eq(published),
                )))
                .await?;
                Ok(old_post)
            }
        }
    }

    async fn delete_post(
        &self,
        ctx: &JuniperContext<State>,
        id: i32,
    ) -> FieldResult<Post> {
        use crate::schema::posts::dsl::posts;
        match ctx.first(posts.find(id)).await? {
            None => Err(FieldError::new(
                "post not found",
                graphql_value!({ "status": 404, "id": id }),
            )),
            Some(old_post) => {
                ctx.execute(diesel::delete(posts.find(id))).await?;
                Ok(old_post)
            }
        }
    }
}

async fn play_ground(ctx: &mut Context<State>) -> roa::Result {
    ctx.write_text(juniper::http::playground::playground_source("/api"));
    ctx.resp
        .insert(header::CONTENT_TYPE, "text/html; charset=utf8")?;
    Ok(())
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    pretty_env_logger::init();
    let router = Router::new().on("/", get(play_ground)).on(
        "/api",
        allow(
            [Method::GET, Method::POST],
            GraphQL(RootNode::new(Query, Mutation)),
        ),
    );
    let app = App::new(create_pool()?)
        .gate(logger)
        .end(router.routes("/")?);
    app.listen("127.0.0.1:8000", |addr| {
        info!("Server is listening on {}", addr)
    })?
    .await?;
    Ok(())
}
