use crate::{Db, Event, Task};

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    bolero::generator::TypeGenerator,
    serde::Deserialize,
    serde::Serialize,
)]
pub enum Action {
    NewTask(Task, String), // task, initial top-comment
    NewEvent(Event),
}

impl Action {
    /// Assumes the action's owner is
    pub async fn is_authorized<D: Db>(&self, db: &mut D) -> anyhow::Result<bool> {
        match self {
            Action::NewTask(t, _) => Ok(t.owner_id == db.current_user()),
            Action::NewEvent(e) => e.is_authorized(db).await,
        }
    }
}
