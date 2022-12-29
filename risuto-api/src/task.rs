use uuid::Uuid;

use crate::{Time, UserId};

#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub struct TaskId(pub Uuid);

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Task {
    pub id: TaskId,
    pub owner_id: UserId,
    pub date: Time,

    pub initial_title: String,
}
