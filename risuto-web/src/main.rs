use risuto_api::*;
use std::collections::HashMap;
use yew::prelude::*;

fn main() {
    tracing_wasm::set_as_global_default();
    yew::start_app::<App>();
}

async fn fetch_db_dump() -> reqwest::Result<DbDump> {
    reqwest::Client::new()
        .get("http://localhost:8000/api/fetch-unarchived") // TODO
        .basic_auth("user1", Some("pass1")) // TODO
        .send()
        .await?
        .json()
        .await
}

enum AppMsg {
    ReceivedDb(DbDump),
    TaskSetDone(TaskId, bool),
}

struct App {
    db: DbDump,
    initial_load_completed: bool,
}

impl Component for App {
    type Message = AppMsg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_future(async move {
            let db: DbDump = loop {
                match fetch_db_dump().await {
                    Ok(db) => break db,
                    Err(e) if e.is_timeout() => continue,
                    // TODO: at least handle unauthorized error
                    _ => panic!("failed to fetch db dump"), // TODO: should eg be a popup
                }
            };
            AppMsg::ReceivedDb(db)
        });
        let users = HashMap::new();
        let tags = HashMap::new();
        let tasks = HashMap::new();
        Self {
            db: DbDump { users, tags, tasks },
            initial_load_completed: false,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            AppMsg::ReceivedDb(db) => {
                self.db = db;
                self.initial_load_completed = true;
                true
            }
            AppMsg::TaskSetDone(id, done) => {
                // TODO: RPC to set task as done
                if let Some(t) = self.db.tasks.get_mut(&id) {
                    t.is_done = done;
                }
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let loading_banner =
            (!self.initial_load_completed).then(|| html! { <h1>{ "Loading..." }</h1> });
        let tasks = self
            .db
            .tasks
            .iter()
            .map(|(id, task)| (*id, task.clone()))
            .collect::<Vec<_>>();
        let on_done_change = ctx.link().callback(|(id, is_done)| AppMsg::TaskSetDone(id, is_done));
        html! {
            <>
                {for loading_banner}
                <h1>{ "Tasks" }</h1>
                <ul class="list-group">
                    <TaskList tasks={tasks} {on_done_change} />
                </ul>
            </>
        }
    }
}

#[derive(Clone, PartialEq, Properties)]
struct TaskListProps {
    tasks: Vec<(TaskId, Task)>,
    on_done_change: Callback<(TaskId, bool)>,
}

#[function_component(TaskList)]
fn task_list(p: &TaskListProps) -> Html {
    p.tasks
        .iter()
        .map(|(id, t)| {
            let on_done_change = {
                let on_done_change = p.on_done_change.clone();
                let id = *id;
                let is_done = t.is_done;
                Callback::from(move |_| on_done_change.emit((id, !is_done)))
            };
            html! {
                <li class="list-group-item">
                    { &t.current_title }{ "(owned by " }{ t.owner.0 }{ ")" }
                    {"is currently done:"}{t.is_done}
                    <button onclick={on_done_change}>{ "Done" }</button>
                </li>
            }
        })
        .collect()
}
