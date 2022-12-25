use risuto_api::{Task, TaskId, Time};
use std::{rc::Rc, sync::Arc};
use yew::prelude::*;

use crate::ui;

#[derive(Clone, PartialEq, Properties)]
pub struct TaskListProps {
    pub ref_this: NodeRef,
    pub tasks: Rc<Vec<(TaskId, Arc<Task>)>>,
    pub on_title_change: Callback<(TaskId, String)>,
    pub on_done_change: Callback<(TaskId, bool)>,
    pub on_schedule_for: Callback<(TaskId, Option<Time>)>,
    pub on_blocked_until: Callback<(TaskId, Option<Time>)>,
}

#[function_component(TaskList)]
pub fn task_list(p: &TaskListProps) -> Html {
    // First, build the list items
    let list_items = p.tasks.iter().map(|(id, t)| {
        macro_rules! reform_with_id {
            ( $($evt:ident,)* ) => {
                $(
                    let $evt = {
                        let id = *id;
                        p.$evt.reform(move |val| (id, val))
                    };
                )*
            }
        }
        reform_with_id! {
            on_title_change,
            on_done_change,
            on_schedule_for,
            on_blocked_until,
        }
        html! {
            <ui::TaskListItem
                task={ t.clone() }
                { on_title_change }
                { on_done_change }
                { on_schedule_for }
                { on_blocked_until }
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
