use gloo_storage::{LocalStorage, Storage};
use risuto_api::*;
use std::collections::HashMap;
use uuid::uuid;
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

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct LoginInfo {
    host: String,
    user: String,
    pass: String,
}

enum AppMsg {
    UserLogin(LoginInfo),
    UserLogout,
    ReceivedDb(DbDump),
    TaskSetDone(TaskId, bool),
}

struct App {
    login: Option<LoginInfo>,
    logout: Option<LoginInfo>, // info saved from login info
    db: DbDump,
    initial_load_completed: bool,
}

impl App {
    fn fetch_db_dump(&self, ctx: &Context<Self>) {
        let login = self
            .login
            .clone()
            .expect("called App::fetch_db_dump without a login set");
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
}

impl Component for App {
    type Message = AppMsg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let login = LocalStorage::get("login").ok();
        let this = Self {
            login,
            logout: None,
            db: DbDump {
                owner: UserId(uuid!("00000000-0000-0000-0000-000000000000")),
                users: HashMap::new(),
                tags: HashMap::new(),
                tasks: HashMap::new(),
            },
            initial_load_completed: false,
        };
        if this.login.is_some() {
            this.fetch_db_dump(ctx);
        }
        this
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            AppMsg::UserLogin(login) => {
                LocalStorage::set("login", &login)
                    .expect("failed saving login info to LocalStorage");
                self.login = Some(login);
                self.fetch_db_dump(ctx);
            }
            AppMsg::UserLogout => {
                LocalStorage::delete("login");
                self.logout = self.login.take().map(|mut i| {
                    i.pass = String::new();
                    i
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
                <div class="container">
                    <Login
                        info={self.logout.clone()}
                        on_submit={ctx.link().callback(AppMsg::UserLogin)}
                    />
                </div>
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
        let on_done_change = ctx
            .link()
            .callback(|(id, is_done)| AppMsg::TaskSetDone(id, is_done));
        html! {
            <>
                {for loading_banner}
                <h1>{ "Tags" }</h1>
                <ul>
                    {for self.db.tags.iter().map(|(_id, t)| html! {
                        <li>
                            { for (t.owner != self.db.owner)
                                .then(|| format!("{}:",
                                    self.db.users.get(&t.owner)
                                        .expect("got a tag owned by an user that does not exist").name
                                )) }
                            { &t.name }
                        </li>
                    })}
                </ul>
                <h1>{ "Tasks" }</h1>
                <button onclick={ctx.link().callback(|_| AppMsg::UserLogout)}>
                    { "Logout" }
                </button>
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
                let id = *id;
                let is_done = t.is_done;
                p.on_done_change.reform(move |_| (id, !is_done))
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
