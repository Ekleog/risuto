use async_trait::async_trait;

use crate::{AuthInfo, EventId, TagId, TaskId, Time, UserId};

#[async_trait]
pub trait Db {
    fn current_user(&self) -> UserId;
    async fn auth_info_for(&mut self, t: TaskId) -> anyhow::Result<AuthInfo>;
    async fn list_tags_for(&mut self, t: TaskId) -> anyhow::Result<Vec<TagId>>;
    async fn get_event_info(&mut self, e: EventId) -> anyhow::Result<(UserId, Time, TaskId)>;
    async fn is_top_comment(&mut self, task: TaskId, comment: EventId) -> anyhow::Result<bool>;
}
