use crate::STUB_UUID;

use uuid::Uuid;

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    PartialEq,
    bolero::generator::TypeGenerator,
    serde::Deserialize,
    serde::Serialize,
)]
pub struct UserId(#[generator(bolero::generator::gen_arbitrary())] pub Uuid);

impl UserId {
    pub fn stub() -> UserId {
        UserId(STUB_UUID)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct User {
    pub id: UserId,
    pub name: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct NewUser {
    pub id: UserId,
    pub name: String,
    pub initial_password: String,
}
