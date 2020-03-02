use diesel::Queryable;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Queryable, Serialize, Deserialize)]
pub struct Post {
    pub id: i32,
    pub title: String,
    pub body: String,
    pub published: bool,
}
