use diesel::Queryable;
use serde::Serialize;

#[derive(Debug, Clone, Queryable, Serialize)]
pub struct Post {
    pub id: i32,
    pub title: String,
    pub body: String,
    pub published: bool,
}
