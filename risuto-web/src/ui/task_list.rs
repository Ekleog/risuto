use risuto_api::{Task, TaskId};
use std::rc::Rc;
use yew::prelude::*;

use crate::ui;

#[derive(Clone, PartialEq, Properties)]
pub struct TaskListProps {
    pub ref_this: NodeRef,
    pub tasks: Rc<Vec<(TaskId, Task)>>,
    pub on_done_change: Callback<(TaskId, bool)>,
}

#[function_component(TaskList)]
pub fn task_list(p: &TaskListProps) -> Html {
    // First, build the list items
    let list_items = p.tasks.iter().map(|(id, t)| {
        let on_done_change = {
            let id = *id;
            let is_done = t.is_done;
            p.on_done_change.reform(move |()| (id, !is_done))
        };
        html! {
            <ui::TaskListItem
                task={ t.clone() }
                { on_done_change }
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
