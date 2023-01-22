use crate::{Db, Error, Event, Task, User};

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
    NewUser(User),
    NewTask(
        Task,
        #[generator(bolero::generator::gen_with::<String>().len(0..100usize))] String,
    ), // task, initial top-comment
    NewEvent(Event),
}

impl Action {
    /// Assumes the action's owner is
    pub async fn is_authorized<D: Db>(&self, db: &mut D) -> anyhow::Result<bool> {
        match self {
            Action::NewUser(_) => Ok(false), // Only admin can create a user for now
            Action::NewTask(t, _) => Ok(t.owner_id == db.current_user()),
            Action::NewEvent(e) => e.is_authorized(db).await,
        }
    }

    /// Helper function to check whether the action is valid.
    ///
    /// Note that you should not rely on the fact that an Action struct is "valid" according
    /// to this in order to ensure safety of your code. (parsing is better than validation)
    pub fn validate(&self) -> Result<(), Error> {
        match self {
            Action::NewUser(_) => Err(Error::PermissionDenied),
            Action::NewTask(t, top_comm) => {
                crate::validate_string(&top_comm)?;
                t.validate()
            }
            Action::NewEvent(e) => e.validate(),
        }
    }
}
