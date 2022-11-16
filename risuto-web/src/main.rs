use futures::{channel::mpsc, FutureExt};
use gloo_storage::{LocalStorage, Storage};
use risuto_api::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

mod api;

mod ui;

fn main() {
    tracing_wasm::set_as_global_default();
    yew::start_app::<App>();
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
            .expect("called App::fetch_db_dump without a login set")
            .info;
        ctx.link()
            .send_future(api::fetch_db_dump(self.client.clone(), login).map(AppMsg::ReceivedDb));
    }

    fn got_login_info(&mut self, ctx: &Context<Self>, info: LoginInfo) {
        let (event_submitter, event_receiver) = mpsc::unbounded();
        spawn_local(api::handle_event_submissions(
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
                for (t, e) in e.untrusted_task_event_list().into_iter() {
                    match self.db.tasks.get_mut(&t) {
                        None => tracing::warn!(evt=?e, "got event for task not in db"),
                        Some(t) => {
                            t.add_event(e);
                            t.refresh_metadata();
                        }
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
                    <ui::Login
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
            ctx.link().callback(move |(task, now_done)| {
                AppMsg::NewTaskEvent(NewEvent::now(
                    owner,
                    NewEventContents::SetDone { task, now_done },
                ))
            })
        };
        let current_tag = self.tag.as_ref().and_then(|t| self.db.tags.get(t)).cloned();
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
                { for loading_banner }
                <div class="row">
                    <nav class="col-md-2 sidebar">
                        <h1>{ "Tags" }</h1>
                        <ui::TagList
                            tags={self.db.tags.clone()}
                            current_user={self.db.owner}
                            active={self.tag}
                            on_select_tag={ctx.link().callback(|id| AppMsg::SetTag(id))}
                        />
                    </nav>
                    <main class="col-md-9 m-5">
                        <h1>{ "Tasks for tag " }{ current_tag.map(|t| t.name).unwrap_or_else(|| String::from(":untagged")) }</h1>
                        <button onclick={ctx.link().callback(|_| AppMsg::UserLogout)}>
                            { "Logout" }
                        </button>
                        <ui::TaskList tasks={tasks} {on_done_change} />
                    </main>
                </div>
            </div>
        }
    }
}
