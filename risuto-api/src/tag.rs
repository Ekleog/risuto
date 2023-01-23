use uuid::Uuid;

use crate::{UserId, STUB_UUID};

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    arbitrary::Arbitrary,
    bolero::generator::TypeGenerator,
    serde::Deserialize,
    serde::Serialize,
)]
pub struct TagId(#[generator(bolero::generator::gen_arbitrary())] pub Uuid);

impl TagId {
    pub fn stub() -> TagId {
        TagId(STUB_UUID)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Tag {
    pub id: TagId,
    pub owner_id: UserId,
    pub name: String,
    pub archived: bool,
}
