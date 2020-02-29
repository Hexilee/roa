use diesel::Queryable;
use juniper::GraphQLObject;
use serde::Deserialize;

#[derive(Debug, Clone, Queryable, Deserialize, GraphQLObject)]
#[graphql(description = "A post")]
pub struct Post {
    pub id: i32,
    pub title: String,
    pub body: String,
    pub published: bool,
}
