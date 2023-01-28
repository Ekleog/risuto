use crate::{auth::BCRYPT_POW_COST, Error, STUB_UUID};

use uuid::Uuid;

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    bolero::generator::TypeGenerator,
    serde::Deserialize,
    serde::Serialize,
)]
pub struct UserId(#[generator(bolero::gen_arbitrary())] pub Uuid);

impl UserId {
    pub fn stub() -> UserId {
        UserId(STUB_UUID)
    }
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    bolero::generator::TypeGenerator,
    serde::Deserialize,
    serde::Serialize,
)]
pub struct User {
    pub id: UserId,
    pub name: String,
}

#[derive(Clone, Debug, bolero::generator::TypeGenerator, serde::Deserialize, serde::Serialize)]
pub struct NewUser {
    pub id: UserId,
    #[generator(bolero::gen_with::<String>().len(1..100usize))]
    pub name: String,
    #[generator(bolero::gen_with::<String>().len(1..100usize))]
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

    /// Helper function to check whether the user name is valid.
    ///
    /// Note that you should not rely on the fact that a NewUser struct is "valid" according
    /// to this in order to ensure safety of your code. (parsing is better than validation)
    pub fn validate(&self) -> Result<(), Error> {
        crate::validate_string(&self.name)?;
        crate::validate_string(&self.initial_password_hash)?;
        if self.name.chars().any(|c| {
            !((c >= 'a' && c <= 'z')
                || (c >= 'A' && c <= 'Z')
                || (c >= '0' && c <= '9')
                || c == '_'
                || c == '-')
        }) {
            return Err(Error::InvalidName(self.name.clone()));
        }
        Ok(())
    }
}
