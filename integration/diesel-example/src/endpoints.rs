use crate::data_object::PostData;
use crate::models::*;
use crate::schema::posts::dsl::{self, posts};
use crate::State;
use async_std::task::spawn;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use roa::core::{throw, Context, Result, StatusCode};
use roa::preload::*;
use roa::router::Router;
use roa_diesel::{AsyncQuery, Result as WrapResult, WrapConnection, WrapError};

pub fn post_router() -> Router<State> {
    let mut router = Router::new();
    router.post("/", create_post);
    router.get("/:id", get_post);
    router.put("/:id", update_post);
    router.delete("/:id", delete_post);
    router
}

async fn find_post(
    conn: WrapConnection<SqliteConnection>,
    id: i32,
) -> WrapResult<Option<Post>> {
    posts
        .find(id)
        .filter(dsl::published.eq(true))
        .first_async::<Post>(conn)
        .await
}

async fn create_post(mut ctx: Context<State>) -> Result {
    let data: PostData = ctx.read_json().await?;
    let conn = ctx.state().await.get().await?;
    let post = spawn(async move {
        conn.transaction::<Post, WrapError, _>(|| {
            diesel::insert_into(crate::schema::posts::table)
                .values(&data)
                .execute(&conn)?;
            Ok(posts.order(dsl::id.desc()).first(&conn)?)
        })
    })
    .await?;
    ctx.resp_mut().await.status = StatusCode::CREATED;
    ctx.write_json(&post).await
}

async fn get_post(mut ctx: Context<State>) -> Result {
    let id: i32 = ctx.must_param("id").await?.parse()?;
    let conn = ctx.state().await.get().await?;
    match find_post(conn, id).await? {
        None => throw!(StatusCode::NOT_FOUND, &format!("post({}) not found", id)),
        Some(post) => {
            let data: PostData = post.into();
            ctx.write_json(&data).await
        }
    }
}

async fn update_post(mut ctx: Context<State>) -> Result {
    let id: i32 = ctx.must_param("id").await?.parse()?;
    let PostData {
        title,
        body,
        published,
    } = ctx.read_json().await?;

    let conn = ctx.state().await.get().await?;
    match find_post(conn, id).await? {
        None => throw!(StatusCode::NOT_FOUND, &format!("post({}) not found", id)),
        Some(post) => {
            let old_data: PostData = post.into();
            diesel::update(posts.find(id))
                .set((
                    dsl::title.eq(title),
                    dsl::body.eq(body),
                    dsl::published.eq(published),
                ))
                .execute_async(ctx.state().await.get().await?)
                .await?;
            ctx.write_json(&old_data).await
        }
    }
}

async fn delete_post(mut ctx: Context<State>) -> Result {
    let id: i32 = ctx.must_param("id").await?.parse()?;
    let conn = ctx.state().await.get().await?;
    match find_post(conn, id).await? {
        None => throw!(StatusCode::NOT_FOUND, &format!("post({}) not found", id)),
        Some(post) => {
            let old_data: PostData = post.into();
            diesel::delete(posts.find(id))
                .execute_async(ctx.state().await.get().await?)
                .await?;
            ctx.write_json(&old_data).await
        }
    }
}
