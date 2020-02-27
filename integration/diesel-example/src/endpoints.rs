use crate::data_object::PostData;
use crate::models::*;
use crate::schema::posts::dsl::{self, posts};
use crate::State;
use diesel::prelude::*;
use roa::http::StatusCode;
use roa::preload::*;
use roa::router::Router;
use roa::{throw, Context, Result};
use roa_diesel::{AsyncPool, Result as WrapResult, SqlQuery, WrapError};

pub fn post_router() -> Router<State> {
    let mut router = Router::new();
    router.post("/", create_post);
    router.get("/:id", get_post);
    router.put("/:id", update_post);
    router.delete("/:id", delete_post);
    router
}

async fn find_post(ctx: &Context<State>, id: i32) -> WrapResult<Option<Post>> {
    ctx.first(posts.find(id).filter(dsl::published.eq(true)))
        .await
}

async fn create_post(mut ctx: Context<State>) -> Result {
    let data: PostData = ctx.read_json().await?;
    let conn = ctx.get_conn().await?;
    let post = ctx
        .exec()
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
    match find_post(&ctx, id).await? {
        None => throw!(StatusCode::NOT_FOUND, &format!("post({}) not found", id)),
        Some(post) => {
            let data: PostData = post.into();
            ctx.write_json(&data)
        }
    }
}

async fn update_post(mut ctx: Context<State>) -> Result {
    let id: i32 = ctx.must_param("id")?.parse()?;
    let PostData {
        title,
        body,
        published,
    } = ctx.read_json().await?;

    match find_post(&ctx, id).await? {
        None => throw!(StatusCode::NOT_FOUND, &format!("post({}) not found", id)),
        Some(post) => {
            let old_data: PostData = post.into();
            ctx.execute(diesel::update(posts.find(id)).set((
                dsl::title.eq(title),
                dsl::body.eq(body),
                dsl::published.eq(published),
            )))
            .await?;
            ctx.write_json(&old_data)
        }
    }
}

async fn delete_post(mut ctx: Context<State>) -> Result {
    let id: i32 = ctx.must_param("id")?.parse()?;
    match find_post(&ctx, id).await? {
        None => throw!(StatusCode::NOT_FOUND, &format!("post({}) not found", id)),
        Some(post) => {
            let old_data: PostData = post.into();
            ctx.execute(diesel::delete(posts.find(id))).await?;
            ctx.write_json(&old_data)
        }
    }
}
