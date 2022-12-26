use risuto_api::{DbDump, EventData, TagId, Task, TaskId};
use std::{rc::Rc, sync::Arc};
use yew::prelude::*;

use crate::ui;

#[derive(Clone, PartialEq, Properties)]
pub struct TaskListProps {
    pub ref_this: NodeRef,
    pub db: Rc<DbDump>,
    pub current_tag: Option<TagId>,
    pub tasks: Rc<Vec<(TaskId, Arc<Task>)>>,
    pub on_event: Callback<(TaskId, EventData)>,
}

#[function_component(TaskList)]
pub fn task_list(p: &TaskListProps) -> Html {
    // First, build the list items
    let list_items = p.tasks.iter().map(|(id, t)| {
        let on_event = {
            let id = *id;
            p.on_event.reform(move |data| (id, data))
        };
        html! {
            <ui::TaskListItem
                task={ t.clone() }
                db={ p.db.clone() }
                current_tag={ p.current_tag.clone() }
                { on_event }
            />
        }
    });

    // Then, put everything together
    html! {
        <ul ref={p.ref_this.clone()} class="task-list list-group">
            { for list_items }
        </ul>
    }
}
