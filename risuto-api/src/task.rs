use uuid::Uuid;

use crate::{Time, UserId, STUB_UUID};

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
pub struct TaskId(#[generator(bolero::generator::gen_arbitrary())] pub Uuid);

impl TaskId {
    pub fn stub() -> TaskId {
        TaskId(STUB_UUID)
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
pub struct Task {
    pub id: TaskId,
    pub owner_id: UserId,
    #[generator(bolero::generator::gen_arbitrary())]
    pub date: Time,

    pub initial_title: String,
}
