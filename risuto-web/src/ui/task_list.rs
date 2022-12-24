use risuto_api::{Task, TaskId};
use std::{rc::Rc, sync::Arc};
use yew::prelude::*;

use crate::ui;

#[derive(Clone, PartialEq, Properties)]
pub struct TaskListProps {
    pub ref_this: NodeRef,
    pub tasks: Rc<Vec<(TaskId, Arc<Task>)>>,
    pub on_title_change: Callback<(TaskId, String)>,
    pub on_done_change: Callback<(TaskId, bool)>,
}

#[function_component(TaskList)]
pub fn task_list(p: &TaskListProps) -> Html {
    // First, build the list items
    let list_items = p.tasks.iter().map(|(id, t)| {
        let on_title_change = {
            let id = *id;
            p.on_title_change.reform(move |new_title| (id, new_title))
        };
        let on_done_change = {
            let id = *id;
            let is_done = t.is_done;
            p.on_done_change.reform(move |()| (id, !is_done))
        };
        html! {
            <ui::TaskListItem
                task={ t.clone() }
                { on_title_change }
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
