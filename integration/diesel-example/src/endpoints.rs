use crate::data_object::NewPost;
use crate::models::*;
use crate::schema::posts::dsl::{self, posts};
use crate::State;
use diesel::prelude::*;
use roa::http::StatusCode;
use roa::preload::*;
use roa::router::Router;
use roa::{throw, Context, Result};
use roa_diesel::{AsyncPool, SqlQuery, WrapError};

pub fn post_router() -> Router<State> {
    let mut router = Router::new();
    router.post("/", create_post);
    router.get("/:id", get_post);
    router.put("/:id", update_post);
    router.delete("/:id", delete_post);
    router
}

async fn create_post(mut ctx: Context<State>) -> Result {
    let data: NewPost = ctx.read_json().await?;
    let conn = ctx.get_conn().await?;
    let post = ctx
        .exec
        .spawn_blocking(move || {
            conn.transaction::<Post, WrapError, _>(|| {
                diesel::insert_into(crate::schema::posts::table)
                    .values(&data)
                    .execute(&conn)?;
                Ok(posts.order(dsl::id.desc()).first(&conn)?)
            })
        })
        .await?;
    ctx.resp_mut().status = StatusCode::CREATED;
    ctx.write_json(&post)
}

async fn get_post(mut ctx: Context<State>) -> Result {
    let id: i32 = ctx.must_param("id")?.parse()?;
    match ctx
        .first::<Post, _>(posts.find(id).filter(dsl::published.eq(true)))
        .await?
    {
        None => throw!(StatusCode::NOT_FOUND, &format!("post({}) not found", id)),
        Some(post) => ctx.write_json(&post),
    }
}

async fn update_post(mut ctx: Context<State>) -> Result {
    let id: i32 = ctx.must_param("id")?.parse()?;
    let NewPost {
        title,
        body,
        published,
    } = ctx.read_json().await?;

    match ctx.first::<Post, _>(posts.find(id)).await? {
        None => throw!(StatusCode::NOT_FOUND, &format!("post({}) not found", id)),
        Some(post) => {
            ctx.execute(diesel::update(posts.find(id)).set((
                dsl::title.eq(title),
                dsl::body.eq(body),
                dsl::published.eq(published),
            )))
            .await?;
            ctx.write_json(&post)
        }
    }
}

async fn delete_post(mut ctx: Context<State>) -> Result {
    let id: i32 = ctx.must_param("id")?.parse()?;
    match ctx.first::<Post, _>(posts.find(id)).await? {
        None => throw!(StatusCode::NOT_FOUND, &format!("post({}) not found", id)),
        Some(post) => {
            ctx.execute(diesel::delete(posts.find(id))).await?;
            ctx.write_json(&post)
        }
    }
}
