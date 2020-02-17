use diesel::Queryable;

#[derive(Debug, Clone, Queryable)]
pub struct Post {
    pub id: i32,
    pub title: String,
    pub body: String,
    pub published: bool,
}
