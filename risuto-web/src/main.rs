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
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        let loading_banner =
            (!self.initial_load_completed).then(|| html! { <h1>{ "Loading..." }</h1> });
        let tasks = self
            .db
            .tasks
            .iter()
            .map(|(id, task)| (*id, task.clone()))
            .collect::<Vec<_>>();
        html! {
            <>
                {for loading_banner}
                <h1>{ "Tasks" }</h1>
                <ul class="list-group">
                    <TaskList tasks={tasks} />
                </ul>
            </>
        }
    }
}

#[derive(Clone, PartialEq, Properties)]
struct TaskListProps {
    tasks: Vec<(TaskId, Task)>,
}

#[function_component(TaskList)]
fn task_list(TaskListProps { tasks }: &TaskListProps) -> Html {
    tasks.iter().map(|(_, t)| html! {
        <li class="list-group-item">{ format!("{} (owned by {})", t.current_title, t.owner.0)}</li>
    }).collect()
}
