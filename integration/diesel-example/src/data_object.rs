use crate::schema::posts;
use serde::Deserialize;

// for both transfer and access
#[derive(Debug, Insertable, Deserialize)]
#[table_name = "posts"]
pub struct NewPost {
    pub title: String,
    pub body: String,
    pub published: bool,
}
