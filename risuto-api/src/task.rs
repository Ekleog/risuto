use uuid::Uuid;

use crate::{Error, EventId, Time, UserId, STUB_UUID};

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
pub struct TaskId(#[generator(bolero::gen_arbitrary())] pub Uuid);

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
    #[generator(bolero::gen_arbitrary())]
    pub date: Time,

    #[generator(bolero::gen_with::<String>().len(0..100usize))]
    pub initial_title: String,
    pub top_comment_id: EventId,
}

impl Task {
    pub fn validate(&self) -> Result<(), Error> {
        crate::validate_time(&self.date)?;
        crate::validate_string(&self.initial_title)
    }
}
