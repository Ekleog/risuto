use crate::{Event, Task};

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum Action {
    NewTask(Task),
    NewEvent(Event),
}
