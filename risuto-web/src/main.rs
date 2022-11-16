use std::{collections::VecDeque, future::Future, pin::Pin};

use futures::{
    channel::mpsc::{self},
    future::{self},
    select, FutureExt, StreamExt,
};
use gloo_storage::{LocalStorage, Storage};
use risuto_api::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

mod login;
use login::Login;

fn main() {
    tracing_wasm::set_as_global_default();
    yew::start_app::<App>();
}

async fn fetch_db_dump(client: &reqwest::Client, login: &LoginInfo) -> reqwest::Result<DbDump> {
    client
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

#[derive(Clone, Debug)]
pub struct LoginData {
    info: LoginInfo,
    event_submitter: mpsc::UnboundedSender<NewEvent>,
}

enum AppMsg {
    UserLogin(LoginInfo),
    UserLogout,
    ReceivedDb(DbDump),
    SetTag(Option<TagId>),
    NewTaskEvent(NewEvent),
}

struct App {
    client: reqwest::Client,
    login: Option<LoginData>,
    logout: Option<LoginInfo>, // info saved from login info
    db: DbDump,
    initial_load_completed: bool,
    tag: Option<TagId>,
}

impl App {
    fn new() -> App {
        App {
            client: reqwest::Client::new(),
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
        let client = self.client.clone();
        ctx.link().send_future(async move {
            let db: DbDump = loop {
                match fetch_db_dump(&client, &login.info).await {
                    Ok(db) => break db,
                    Err(e) if e.is_timeout() => continue,
                    // TODO: at least handle unauthorized error
                    _ => panic!("failed to fetch db dump"), // TODO: should eg be a popup
                }
            };
            AppMsg::ReceivedDb(db)
        });
    }

    fn got_login_info(&mut self, ctx: &Context<Self>, info: LoginInfo) {
        let (event_submitter, event_receiver) = mpsc::unbounded();
        spawn_local(handle_event_submissions(
            self.client.clone(),
            info.clone(),
            event_receiver,
        ));
        self.login = Some(LoginData {
            info,
            event_submitter,
        });
        self.fetch_db_dump(ctx);
    }
}

impl Component for App {
    type Message = AppMsg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let mut this = App::new();
        if let Ok(info) = LocalStorage::get("login") {
            this.got_login_info(ctx, info);
        }
        this
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            AppMsg::UserLogin(info) => {
                LocalStorage::set("login", &info)
                    .expect("failed saving login info to LocalStorage");
                self.got_login_info(ctx, info);
            }
            AppMsg::UserLogout => {
                LocalStorage::delete("login");
                // TODO: also clear saved outgoing queue, and warn the user upon logout that unsynced changes will be lost
                let mut this = App::new();
                this.logout = self.login.take().map(|i| LoginInfo {
                    host: i.info.host,
                    user: i.info.user,
                    pass: String::new(),
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
                self.login
                    .as_mut()
                    .expect("got NewTaskEvent without a login configured")
                    .event_submitter
                    .unbounded_send(e.clone())
                    .expect("failed sending local event to event submitter");
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
            tasks.sort_unstable_by_key(|(meta, id, _)| (meta.priority, *id));
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

async fn send_event(client: &reqwest::Client, login: &LoginInfo, event: NewEvent) {
    loop {
        let res = client
            .post(format!("{}/api/submit-event", login.host))
            .basic_auth(&login.user, Some(&login.pass))
            .json(&event)
            .send()
            .await;
        match res {
            // TODO: panicking on server message is Bad(tm)
            Ok(r) if r.status().is_success() => break,
            Ok(r) => panic!("got non-successful response to event submission: {:?}", r),
            Err(e) if e.is_timeout() => continue,
            Err(e) => panic!("got reqwest error {:?}", e),
        }
    }
}

async fn handle_event_submissions(
    client: reqwest::Client,
    login: LoginInfo,
    queue: mpsc::UnboundedReceiver<NewEvent>,
) {
    let mut queue = queue.fuse();
    let mut to_send = LocalStorage::get("queue").ok().unwrap_or(VecDeque::new());
    // TODO: to_send should be exposed from the UI
    let mut currently_sending = false;
    let mut current_send =
        (Box::pin(future::pending()) as Pin<Box<dyn Future<Output = ()>>>).fuse();
    loop {
        select! {
            e = queue.next() => {
                match e {
                    None => break,
                    Some(e) => {
                        to_send.push_back(e);
                        LocalStorage::set("queue", &to_send)
                            .expect("failed saving queue to local storage");
                    }
                }
            }
            _ = current_send => {
                to_send.pop_front();
                LocalStorage::set("queue", &to_send)
                    .expect("failed saving queue to local storage");
                currently_sending = false;
            }
        }
        if !currently_sending && !to_send.is_empty() {
            current_send = (Box::pin(send_event(&client, &login, to_send[0].clone()))
                as Pin<Box<dyn Future<Output = ()>>>)
                .fuse();
            currently_sending = true;
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
