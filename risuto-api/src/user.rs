use crate::{auth::BCRYPT_POW_COST, STUB_UUID};

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
    pub initial_password_hash: String,
}

impl NewUser {
    pub fn new(id: UserId, name: String, initial_password: String) -> NewUser {
        NewUser {
            id,
            name,
            initial_password_hash: bcrypt::hash(initial_password, BCRYPT_POW_COST)
                .expect("failed bcrypt hashing password"),
        }
    }
}
