use risuto_api::*;
use std::collections::HashMap;
use yew::prelude::*;

mod login;
use login::Login;

fn main() {
    tracing_wasm::set_as_global_default();
    yew::start_app::<App>();
}

async fn fetch_db_dump(login: &LoginInfo) -> reqwest::Result<DbDump> {
    reqwest::Client::new()
        .get(format!("{}/api/fetch-unarchived", login.host))
        .basic_auth(&login.user, Some(&login.pass))
        .send()
        .await?
        .json()
        .await
}

#[derive(Clone)]
pub struct LoginInfo {
    host: String,
    user: String,
    pass: String,
}

enum AppMsg {
    UserLogin(LoginInfo),
    ReceivedDb(DbDump),
    TaskSetDone(TaskId, bool),
}

struct App {
    login: Option<LoginInfo>,
    db: DbDump,
    initial_load_completed: bool,
}

impl Component for App {
    type Message = AppMsg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            login: None,
            db: DbDump {
                users: HashMap::new(),
                tags: HashMap::new(),
                tasks: HashMap::new(),
            },
            initial_load_completed: false,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            AppMsg::UserLogin(login) => {
                self.login = Some(login.clone());
                ctx.link().send_future(async move {
                    let db: DbDump = loop {
                        match fetch_db_dump(&login).await {
                            Ok(db) => break db,
                            Err(e) if e.is_timeout() => continue,
                            // TODO: at least handle unauthorized error
                            _ => panic!("failed to fetch db dump"), // TODO: should eg be a popup
                        }
                    };
                    AppMsg::ReceivedDb(db)
                });
            }
            AppMsg::ReceivedDb(db) => {
                self.db = db;
                self.initial_load_completed = true;
            }
            AppMsg::TaskSetDone(id, done) => {
                // TODO: RPC to set task as done
                if let Some(t) = self.db.tasks.get_mut(&id) {
                    t.is_done = done;
                }
            }
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        if self.login.is_none() {
            return html! {
                <Login on_submit={ctx.link().callback(AppMsg::UserLogin)} />
            };
        }
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
