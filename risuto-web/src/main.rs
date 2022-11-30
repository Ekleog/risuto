use futures::{
    channel::{mpsc, oneshot},
    executor::block_on,
    FutureExt,
};
use gloo_storage::{LocalStorage, Storage};
use risuto_api::*;
use std::{cell::RefCell, rc::Rc};
use ui::TaskOrderChangeEvent;
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
    token: AuthToken,
}

#[derive(Clone, Debug)]
pub struct LoginData {
    info: LoginInfo,
    event_submitter: mpsc::UnboundedSender<NewEvent>,
    feed_canceller: Rc<RefCell<oneshot::Receiver<()>>>,
}

pub enum AppMsg {
    UserLogin(LoginInfo),
    UserLogout,
    ReceivedDb(DbDump),
    SetTag(Option<TagId>),
    NewUserEvent(NewEvent),
    NewNetworkEvent(NewEvent),
}

pub struct App {
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
        // Connect to websocket event feed
        let feed_sender = ctx.link().clone();
        let (feed_cancel_receiver, feed_canceller) = oneshot::channel();
        spawn_local(api::start_event_feed(
            info.clone(),
            feed_sender,
            feed_cancel_receiver,
        ));

        // Prepare thread handling event submission
        let (event_submitter, event_receiver) = mpsc::unbounded();
        spawn_local(api::handle_event_submissions(
            self.client.clone(),
            info.clone(),
            event_receiver,
        ));

        // Record login info
        self.login = Some(LoginData {
            info,
            event_submitter,
            feed_canceller: Rc::new(RefCell::new(feed_canceller)),
        });

        // Finally, fetch a DB dump from the server
        self.fetch_db_dump(ctx);
    }

    fn logout(&mut self) {
        // TODO: warn the user upon logout that unsynced changes will be lost
        if let Some(ref l) = self.login {
            spawn_local(api::unauth(
                self.client.clone(),
                l.info.host.clone(),
                l.info.token,
            ));
        }
        let mut this = App::new();
        this.logout = self.login.take().map(|i| {
            i.feed_canceller.borrow_mut().close(); // This should be unneeded as it closes on drop, but better safe than sorry
            LoginInfo {
                host: i.info.host,
                user: i.info.user,
                token: AuthToken::stub(),
            }
        }); // info saved from login info
        *self = this;
    }

    fn handle_new_event(&mut self, e: NewEvent) {
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

    fn current_task_list(&self) -> (Rc<Vec<(TaskId, Task)>>, Rc<Vec<(TaskId, Task)>>) {
        let tasks = self.db.tasks.iter();
        let (backlog, normal) = if let Some(tag) = self.tag {
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
                .partition(|(_, t)| t.current_tags.get(&tag).unwrap().backlog)
        } else {
            (
                Vec::new(),
                tasks
                    .filter(|(_, task)| task.current_tags.len() == 0)
                    .map(|(id, task)| (*id, task.clone()))
                    .collect(),
            )
        };
        (Rc::new(normal), Rc::new(backlog))
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
                self.logout();
                LocalStorage::delete("login");
                LocalStorage::delete("queue");
            }
            AppMsg::ReceivedDb(db) => {
                self.db = db;
                self.initial_load_completed = true;
                if self.tag == Some(TagId::stub()) {
                    self.tag = Some(
                        self.db
                            .tags
                            .iter()
                            .find(|(_, t)| t.0.name == "today")
                            .expect("found no tag named 'today'")
                            .0
                            .clone(),
                    );
                }
            }
            AppMsg::SetTag(id) => {
                self.tag = id;
            }
            AppMsg::NewUserEvent(e) => {
                // Sanity-check that we're allowed to submit the event before adding it to the queue
                assert!(
                    block_on(e.is_authorized(&mut self.db)).expect("checking is_authorized on local db dump"),
                    "Submitted userevent that is not authorized. The button should have been disabled! {:?}",
                    e,
                );

                // Submit the event to the upload queue and update our state
                self.login
                    .as_mut()
                    .expect("got NewTaskEvent without a login configured")
                    .event_submitter
                    .unbounded_send(e.clone())
                    .expect("failed sending local event to event submitter");
                self.handle_new_event(e);
            }
            AppMsg::NewNetworkEvent(e) => self.handle_new_event(e),
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        if self.login.is_none() {
            return html! {
                <div class="container">
                    <ui::Login
                        info={self.logout.clone()}
                        on_authed={ctx.link().callback(AppMsg::UserLogin)}
                    />
                </div>
            };
        }
        let loading_banner =
            (!self.initial_load_completed).then(|| html! { <h1>{ "Loading..." }</h1> });
        let current_tag = self.tag.as_ref().and_then(|t| self.db.tags.get(t)).cloned();
        let (tasks_normal, tasks_backlog) = self.current_task_list();
        let on_done_change = {
            let owner = self.db.owner.clone();
            ctx.link().callback(move |(task, now_done)| {
                AppMsg::NewUserEvent(NewEvent::now(
                    owner,
                    NewEventContents::SetDone { task, now_done },
                ))
            })
        };
        let on_order_change = {
            let owner = self.db.owner.clone();
            let tag = self.tag.clone();
            let tasks_normal = tasks_normal.clone();
            let tasks_backlog = tasks_backlog.clone();
            ctx.link().batch_callback(move |e: TaskOrderChangeEvent| {
                let task_id = match e.before.in_backlog {
                    true => tasks_backlog[e.before.index].0,
                    false => tasks_normal[e.before.index].0,
                };
                let mut insert_into = match e.after.in_backlog {
                    true => (*tasks_backlog).clone(),
                    false => (*tasks_normal).clone(),
                };
                if e.before.in_backlog == e.after.in_backlog {
                    insert_into.remove(e.before.index);
                }
                compute_reordering_events(
                    owner,
                    tag.expect("attempted to reorder in untagged list"),
                    task_id,
                    e.after.index,
                    e.after.in_backlog,
                    insert_into,
                )
            })
        };
        html! {
            <div class="container-fluid">
                { for loading_banner }
                <div class="row">
                    <nav class="navbar navbar-expand-sm">
                        <div class="container-fluid">
                            <h1>{ "Tasks for tag " }{ current_tag.map(|t| t.0.name).unwrap_or_else(|| String::from(":untagged")) }</h1>
                            <button onclick={ctx.link().callback(|_| AppMsg::UserLogout)}>
                                { "Logout" }
                            </button>
                        </div>
                    </nav>
                    <nav class="col-md-2 sidebar overflow-scroll">
                        <ui::TagList
                            tags={self.db.tags.clone()}
                            current_user={self.db.owner}
                            active={self.tag}
                            on_select_tag={ctx.link().callback(|id| AppMsg::SetTag(id))}
                        />
                    </nav>
                    <main class="col-md-9 m-5">
                        <ui::TaskList
                            {tasks_normal}
                            {tasks_backlog}
                            {on_done_change}
                            {on_order_change}
                            />
                    </main>
                </div>
            </div>
        }
    }
}

fn compute_reordering_events(
    owner: UserId,
    tag: TagId,
    task: TaskId,
    index: usize,
    into_backlog: bool,
    into: Vec<(TaskId, Task)>,
) -> Vec<AppMsg> {
    macro_rules! evt {
        ( $task:expr, $prio:expr ) => {
            AppMsg::NewUserEvent(NewEvent::now(
                owner,
                NewEventContents::AddTag {
                    task: $task,
                    tag,
                    prio: $prio,
                    backlog: into_backlog,
                },
            ))
        };
    }
    macro_rules! prio {
        ($task:expr) => {
            $task
                .1
                .prio(&tag)
                .expect("computing events reordering with task not in tag")
        };
    }
    // this value was taken after intense finger-based wind-speed-taking
    // basically we can add 2^(64-40) items at the beginning or end this way, and intersperse 40 items in-between other items, all without a redistribution
    const SPACING: i64 = 1 << 40;

    if into.len() == 0 {
        // Easy case: inserting into an empty list
        return vec![evt!(task, 0)];
    }

    if index == 0 {
        // Inserting in the first position
        let first_prio = prio!(into[0]);
        let subtract = match first_prio > i64::MIN + SPACING {
            true => SPACING,
            false => (first_prio - i64::MIN) / 2,
        };
        if subtract > 0 {
            return vec![evt!(task, first_prio - subtract)];
        }
    } else if index == into.len() {
        // Inserting in the last position
        let last_prio = prio!(into[index - 1]);
        let add = match last_prio < i64::MAX - SPACING {
            true => SPACING,
            false => (i64::MAX - last_prio) / 2,
        };
        if add > 0 {
            return vec![evt!(task, last_prio + add)];
        }
    } else {
        // Inserting in-between two elements
        let prio_before = prio!(into[index - 1]);
        let prio_after = prio!(into[index]);
        let add = (prio_after - prio_before) / 2;
        if add > 0 {
            return vec![evt!(task, prio_before + add)];
        }
    }

    // Do a full redistribute
    // TODO: maybe we could only partially redistribute? not sure whether that'd actually be better...
    into[..index]
        .iter()
        .enumerate()
        .map(|(i, (t, _))| evt!(*t, (i as i64).checked_mul(SPACING).unwrap()))
        .chain(std::iter::once(evt!(
            task,
            (index as i64).checked_mul(SPACING).unwrap()
        )))
        .chain(into[index..].iter().enumerate().map(|(i, (t, _))| {
            evt!(
                *t,
                (index as i64 + 1 + i as i64).checked_mul(SPACING).unwrap()
            )
        }))
        .collect()
}
