use risuto_api::{AuthInfo, Tag, TagId, UserId};

pub fn sort_tags(current_user: &UserId, tags: &mut Vec<(&TagId, &(Tag, AuthInfo))>) {
    tags.sort_unstable_by_key(|(id, t)| {
        // TODO: extract into a freestanding fn and reuse for sorting the tag list below tasks
        let is_tag_today = t.0.name == "today";
        let is_owner_me = t.0.owner == *current_user;
        let name = t.0.name.clone();
        let id = (*id).clone();
        (!is_tag_today, !is_owner_me, name, id)
    });
}
