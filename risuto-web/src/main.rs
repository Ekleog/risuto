use gloo_storage::{LocalStorage, Storage};
use risuto_api::*;
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
    SetTag(Option<TagId>),
    NewTaskEvent(NewEvent),
}

struct App {
    login: Option<LoginInfo>,
    logout: Option<LoginInfo>, // info saved from login info
    db: DbDump,
    initial_load_completed: bool,
    tag: Option<TagId>,
}

impl App {
    fn new() -> App {
        App {
            login: None,
            logout: None,
            db: DbDump::stub(),
            initial_load_completed: false,
            tag: Some(TagId::stub()),
        }
    }

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
        let mut this = App::new();
        this.login = LocalStorage::get("login").ok();
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
                let mut this = App::new();
                this.logout = self.login.take().map(|mut i| {
                    i.pass = String::new();
                    i
                }); // info saved from login info
                *self = this;
            }
            AppMsg::ReceivedDb(db) => {
                self.db = db;
                self.initial_load_completed = true;
                if self.tag == Some(TagId::stub()) {
                    self.tag = Some(
                        self.db
                            .tags
                            .iter()
                            .find(|(_, t)| t.name == "today")
                            .expect("found no tag named 'today'")
                            .0
                            .clone(),
                    );
                }
            }
            AppMsg::SetTag(id) => {
                self.tag = id;
            }
            AppMsg::NewTaskEvent(e) => {
                // TODO: RPC to set task as done
                match self.db.tasks.get_mut(&e.task) {
                    None => tracing::warn!(evt=?e, "got event for task not in db"),
                    Some(t) => {
                        t.add_event(e.event);
                        t.refresh_metadata();
                    }
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
        let on_done_change = {
            let owner = self.db.owner.clone();
            ctx.link().callback(move |(id, is_done)| {
                AppMsg::NewTaskEvent(NewEvent::now(id, owner, EventType::SetDone(is_done)))
            })
        };
        let current_tag = self.tag.as_ref().and_then(|t| self.db.tags.get(t)).cloned();
        let mut tag_list = self.db.tags.iter().collect::<Vec<_>>();
        tag_list.sort_unstable_by_key(|(id, t)| {
            let is_tag_today = t.name == "today";
            let is_owner_me = t.owner == self.db.owner;
            let owner_name = self
                .db
                .users
                .get(&t.owner)
                .expect("tag owned by unknown user")
                .name
                .clone();
            let name = t.name.clone();
            let id = (*id).clone();
            (!is_tag_today, !is_owner_me, owner_name, name, id)
        });
        let tag_list = tag_list
            .into_iter()
            .map(|(id, t)| (Some(id.clone()), t.name.clone()))
            .chain(std::iter::once((None, String::from(":untagged"))))
            .map(|(id, tag)| {
                let id = id.clone();
                let a_class = match id == self.tag {
                    true => "nav-link active",
                    false => "nav-link",
                };
                html! {
                    <li class="nav-item border-bottom p-2">
                        <a
                            class={ a_class }
                            href={format!("#tag-{}", tag)}
                            onclick={ctx.link().callback(move |_| AppMsg::SetTag(id))}
                        >
                            { tag }
                        </a>
                    </li>
                }
            });
        let tasks = self.db.tasks.iter();
        let tasks: Vec<_> = if let Some(tag) = self.tag {
            let mut tasks = tasks
                .filter_map(|(id, task)| {
                    task.current_tags
                        .get(&tag)
                        .map(|prio| (prio, *id, task.clone()))
                })
                .collect::<Vec<_>>();
            tasks.sort_unstable_by_key(|(prio, id, _)| (**prio, *id));
            tasks
                .into_iter()
                .map(|(_prio, id, task)| (id, task))
                .collect()
        } else {
            tasks
                .filter(|(_, task)| task.current_tags.len() == 0)
                .map(|(id, task)| (*id, task.clone()))
                .collect()
        };
        html! {
            <div class="container-fluid">
                {for loading_banner}
                <div class="row">
                    <nav class="col-md-2 sidebar">
                        <h1>{ "Tags" }</h1>
                        <ul class="nav flex-column">
                            {for tag_list}
                        </ul>
                    </nav>
                    <main class="col-md-9">
                        <h1>{ "Tasks for tag " }{ current_tag.map(|t| t.name).unwrap_or_else(|| String::from(":untagged")) }</h1>
                        <button onclick={ctx.link().callback(|_| AppMsg::UserLogout)}>
                            { "Logout" }
                        </button>
                        <ul class="task-list list-group">
                            <TaskList tasks={tasks} {on_done_change} />
                        </ul>
                    </main>
                </div>
            </div>
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
            let done_change_button = if t.is_done {
                html! {
                    <button
                        type="button"
                        class="btn bi-btn bi-arrow-counterclockwise"
                        aria-label="Mark undone"
                        onclick={on_done_change}
                    >
                    </button>
                }
            } else {
                html! {
                    <button
                        type="button"
                        class="btn bi-btn bi-check-lg"
                        aria-label="Mark done"
                        onclick={on_done_change}
                    >
                    </button>
                }
            };
            html! {
                <li class="list-group-item d-flex align-items-center">
                    <span class="flex-grow-1">{ &t.current_title }</span>
                    { done_change_button }
                </li>
            }
        })
        .collect()
}
