use risuto_api::{Task, TaskId, EventType};
use std::{rc::Rc, sync::Arc};
use yew::prelude::*;

use crate::ui;

#[derive(Clone, PartialEq, Properties)]
pub struct TaskListProps {
    pub ref_this: NodeRef,
    pub tasks: Rc<Vec<(TaskId, Arc<Task>)>>,
    pub on_event: Callback<(TaskId, EventType)>,
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
