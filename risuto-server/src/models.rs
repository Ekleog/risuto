#[derive(diesel::Queryable)]
pub struct User {
    pub id: usize,
    pub name: String,
    pub password: String,
}
