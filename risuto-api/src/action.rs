use crate::{Event, Task};

#[derive(Clone, Debug, bolero::generator::TypeGenerator, serde::Deserialize, serde::Serialize)]
pub enum Action {
    NewTask(Task),
    NewEvent(Event),
}
