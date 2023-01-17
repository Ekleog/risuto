use crate::Event;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum Action {
    NewEvent(Event),
}
