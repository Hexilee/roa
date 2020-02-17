use crate::models::Post;
use crate::schema::posts;
use serde::{Deserialize, Serialize};

// for both transfer and access
#[derive(Debug, Insertable, Serialize, Deserialize)]
#[table_name = "posts"]
pub struct PostData {
    pub title: String,
    pub body: String,
    pub published: bool,
}

impl From<Post> for PostData {
    fn from(post: Post) -> Self {
        let Post {
            title,
            body,
            published,
            ..
        } = post;
        Self {
            title,
            body,
            published,
        }
    }
}
