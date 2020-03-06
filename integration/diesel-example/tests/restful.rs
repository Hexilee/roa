use diesel_example::models::Post;
use diesel_example::{create_pool, post_router, StdError};
use roa::http::StatusCode;
use roa::preload::*;
use roa::App;
use serde::Serialize;

#[derive(Debug, Serialize, Copy, Clone)]
pub struct NewPost<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub published: bool,
}

impl PartialEq<Post> for NewPost<'_> {
    fn eq(&self, other: &Post) -> bool {
        self.title == other.title
            && self.body == other.body
            && self.published == other.published
    }
}

#[tokio::test]
async fn test() -> Result<(), Box<dyn StdError>> {
    let mut app = App::new(create_pool()?);
    app.gate(post_router().routes("/post")?);
    let (addr, server) = app.run()?;
    async_std::task::spawn(server);
    let base_url = format!("http://{}/post", addr);
    let client = reqwest::Client::new();

    // Not Found
    let resp = client.get(&format!("{}/{}", &base_url, 0)).send().await?;
    assert_eq!(StatusCode::NOT_FOUND, resp.status());

    // Create
    let first_post = NewPost {
        title: "Hello",
        body: "Welcome to roa-diesel",
        published: false,
    };

    let resp = client.post(&base_url).json(&first_post).send().await?;
    assert_eq!(StatusCode::CREATED, resp.status());
    let created_post: Post = resp.json().await?;
    let id = created_post.id;
    assert_eq!(&first_post, &created_post);

    // Post isn't published, get nothing
    let resp = client.get(&format!("{}/{}", &base_url, id)).send().await?;
    assert_eq!(StatusCode::NOT_FOUND, resp.status());

    // Update
    let second_post = NewPost {
        published: true,
        ..first_post
    };
    let resp = client
        .put(&format!("{}/{}", &base_url, id))
        .json(&second_post)
        .send()
        .await?;
    assert_eq!(StatusCode::OK, resp.status());

    // Return old post
    let updated_post: Post = resp.json().await?;
    assert_eq!(&first_post, &updated_post);

    // Get it
    let resp = client.get(&format!("{}/{}", &base_url, id)).send().await?;
    assert_eq!(StatusCode::OK, resp.status());
    let query_post: Post = resp.json().await?;
    assert_eq!(&second_post, &query_post);

    // Delete
    let resp = client
        .delete(&format!("{}/{}", &base_url, id))
        .send()
        .await?;
    assert_eq!(StatusCode::OK, resp.status());
    let deleted_post: Post = resp.json().await?;
    assert_eq!(&second_post, &deleted_post);

    // Post is deleted, get nothing
    let resp = client.get(&format!("{}/{}", &base_url, id)).send().await?;
    assert_eq!(StatusCode::NOT_FOUND, resp.status());
    Ok(())
}
