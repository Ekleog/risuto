use futures::{channel::oneshot, executor::block_on};
use gloo_storage::{LocalStorage, Storage};
use risuto_client::{
    api::{Action, Event, EventData, Order, Search},
    DbDump, Task,
};
use std::{collections::VecDeque, rc::Rc, sync::Arc};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::{
    api, ui,
    ui::{ListType, TaskOrderChangeEvent},
    util, LoginInfo,
};

const KEY_ACTS_PENDING_SUBMISSION: &str = "actions-pending-submission";

#[derive(Clone, PartialEq, Properties)]
pub struct AppProps {
    pub login: LoginInfo,
    pub on_logout: Callback<()>,
}

pub enum AppMsg {
    Logout,

    WebsocketConnected,
    ReceivedDb(DbDump),
    WebsocketDisconnected,

    SetActiveSearch(Search),
    NewUserAction(Action),
    NewNetworkAction(Action),
    ActionSubmissionComplete,
}

#[derive(Clone, PartialEq)]
pub enum ConnState {
    Disconnected,
    WebsocketConnected(VecDeque<Action>),
    Connected,
}

pub struct App {
    db: Rc<DbDump>,
    connection_state: ConnState,
    active_search: Search,
    actions_pending_submission: VecDeque<Action>, // push_back, pop_front
    feed_canceller: oneshot::Receiver<()>,
}

#[derive(Clone)]
struct TaskLists {
    open: Rc<Vec<Arc<Task>>>,
    done: Rc<Vec<Arc<Task>>>,
    backlog: Rc<Vec<Arc<Task>>>,
}

impl App {
    fn locally_insert_new_action(&mut self, a: Action) {
        let db = Rc::make_mut(&mut self.db);
        match a {
            Action::NewUser(u) => {
                db.add_users(vec![u]);
            }
            Action::NewTask(t, top_comm) => {
                let mut task = Task::from(t.clone());
                task.add_event(Event {
                    id: t.top_comment_id,
                    owner_id: t.owner_id,
                    date: t.date,
                    task_id: t.id,
                    data: EventData::AddComment {
                        text: top_comm,
                        parent_id: None,
                    },
                });
                task.refresh_metadata(&db.owner);
                db.tasks.insert(t.id, Arc::new(task));
            }
            Action::NewEvent(e) => match db.tasks.get_mut(&e.task_id) {
                None => tracing::warn!(evt=?e, "got event for task not in db"),
                Some(t) => {
                    let task = Arc::make_mut(t);
                    task.add_event(e);
                    task.refresh_metadata(&db.owner);
                }
            },
        }
    }

    fn current_task_lists(&self) -> TaskLists {
        let mut all_tasks = self
            .db
            .search(&self.active_search)
            .expect("Failed running current active search");
        match self.active_search.order {
            Order::Tag(tag) => {
                let backlog = Rc::new(all_tasks.split_off(all_tasks.partition_point(|t| {
                    !t.current_tags.get(&tag).map(|t| t.backlog).unwrap_or(true)
                })));
                let done = Rc::new(all_tasks.split_off(all_tasks.partition_point(|t| !t.is_done)));
                TaskLists {
                    open: Rc::new(all_tasks),
                    done,
                    backlog,
                }
            }
            Order::Custom(_) => {
                let done = Rc::new(all_tasks.split_off(all_tasks.partition_point(|t| !t.is_done)));
                TaskLists {
                    open: Rc::new(all_tasks),
                    done,
                    backlog: Rc::new(Vec::new()),
                }
            }
            _ => TaskLists {
                open: Rc::new(all_tasks),
                done: Rc::new(Vec::new()),
                backlog: Rc::new(Vec::new()),
            },
        }
    }
}

impl Component for App {
    type Message = AppMsg;
    type Properties = AppProps;

    fn create(ctx: &Context<Self>) -> Self {
        // Connect to websocket event feed
        let feed_sender = ctx.link().clone();
        let (feed_cancel_receiver, feed_canceller) = oneshot::channel();
        spawn_local(api::start_event_feed(
            ctx.props().login.clone(),
            feed_sender,
            feed_cancel_receiver,
        ));

        // Load event submission queue
        let actions_pending_submission: VecDeque<Action> =
            LocalStorage::get(KEY_ACTS_PENDING_SUBMISSION).unwrap_or(VecDeque::new());

        // Start event submission if need be
        if !actions_pending_submission.is_empty() {
            send_action(ctx, actions_pending_submission[0].clone());
        }

        App {
            db: Rc::new(DbDump::stub()),
            connection_state: ConnState::Disconnected,
            active_search: Search::today(util::local_tz()),
            actions_pending_submission,
            feed_canceller,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            AppMsg::Logout => {
                self.feed_canceller.close(); // This should be unneeded as it closes on drop, but better safe than sorry
                LocalStorage::delete(KEY_ACTS_PENDING_SUBMISSION);
                ctx.props().on_logout.emit(());
            }
            AppMsg::WebsocketConnected => {
                self.connection_state = ConnState::WebsocketConnected(VecDeque::new());
            }
            AppMsg::WebsocketDisconnected => {
                self.connection_state = ConnState::Disconnected;
            }
            AppMsg::ReceivedDb(db) => {
                self.db = Rc::new(db);
                for a in self.actions_pending_submission.clone() {
                    self.locally_insert_new_action(a.clone());
                }
                let actions_already_received = match &self.connection_state {
                    ConnState::WebsocketConnected(e) => e.clone(),
                    _ => panic!("received database while websocket is not connected"),
                };
                for a in actions_already_received {
                    self.locally_insert_new_action(a);
                }
                self.connection_state = ConnState::Connected;
            }
            AppMsg::SetActiveSearch(search) => {
                self.active_search = search;
            }
            AppMsg::NewUserAction(a) => {
                tracing::debug!("got new user action {a:?}");
                // Sanity-check that we're allowed to submit the event before adding it to the queue
                assert!(
                    block_on(a.is_authorized(&mut &*self.db)).expect("checking is_authorized on local db dump"),
                    "Submitted user action that is not authorized. The button should have been disabled! Please report a bug. {a:?}",
                );
                tracing::trace!("user action authorized {a:?}");

                // Submit the event to the upload queue and update our state
                self.actions_pending_submission.push_back(a.clone());
                LocalStorage::set(
                    KEY_ACTS_PENDING_SUBMISSION,
                    &self.actions_pending_submission,
                )
                .expect("failed saving queue to local storage");
                tracing::trace!("actions pending submission queue saved");
                if self.actions_pending_submission.len() == 1 {
                    // this is the first event from the queue
                    send_action(ctx, a.clone());
                    tracing::debug!("started action submission with action {a:?}");
                }
                self.locally_insert_new_action(a.clone());
                tracing::debug!("handled new user action {a:?}");
            }
            AppMsg::NewNetworkAction(a) => self.locally_insert_new_action(a),
            AppMsg::ActionSubmissionComplete => {
                self.actions_pending_submission.pop_front();
                LocalStorage::set(
                    KEY_ACTS_PENDING_SUBMISSION,
                    &self.actions_pending_submission,
                )
                .expect("failed saving queue to local storage");
                if !self.actions_pending_submission.is_empty() {
                    let e = self.actions_pending_submission[0].clone();
                    send_action(ctx, e);
                }
            }
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let tasks = self.current_task_lists();

        let on_order_change = {
            let owner = self.db.owner.clone();
            let search = self.active_search.clone();
            let tasks = tasks.clone();
            ctx.link().batch_callback(move |e: TaskOrderChangeEvent| {
                let task_id = match e.before.list {
                    ListType::Open => tasks.open[e.before.index].id,
                    ListType::Done => tasks.done[e.before.index].id,
                    ListType::Backlog => tasks.backlog[e.before.index].id,
                };
                let mut insert_into = match e.after.list {
                    ListType::Open => (*tasks.open).clone(),
                    ListType::Done => (*tasks.done).clone(),
                    ListType::Backlog => (*tasks.backlog).clone(),
                };
                if e.before.list == e.after.list {
                    insert_into.remove(e.before.index);
                }
                let evts = util::compute_reordering_events(
                    owner,
                    &search,
                    task_id,
                    e.after.index,
                    e.after.list.is_backlog(),
                    &insert_into,
                );
                let mut evts = evts
                    .into_iter()
                    .map(Action::NewEvent)
                    .map(AppMsg::NewUserAction)
                    .collect::<Vec<_>>();
                if e.before.list.is_done() != e.after.list.is_done() {
                    evts.push(AppMsg::NewUserAction(Action::NewEvent(Event::now(
                        owner,
                        task_id,
                        EventData::SetDone(e.after.list.is_done()),
                    ))));
                }
                evts
            })
        };

        let current_tag = self.active_search.is_order_tag();
        let user_knows_current_tag = match current_tag {
            None => false,
            Some(t) => match self.db.tags.get(&t) {
                None => false,
                Some(db_tag) => self.active_search == Search::for_tag(db_tag),
            },
        };

        html! {
            <div class="container-fluid vh-100">
                <div class="row h-100">
                    <nav class="col-md-2 sidebar overflow-auto p-0">
                        <ui::SearchList
                            searches={ self.db.searches.clone() }
                            tags={ self.db.tags.clone() }
                            current_user={ self.db.owner }
                            active_search={ self.active_search.id }
                            on_select_search={ ctx.link().callback(AppMsg::SetActiveSearch) }
                        />
                    </nav>
                    <main class="col-md-10 h-100 p-0">
                        <ui::MainView
                            connection_state={ self.connection_state.clone() }
                            actions_pending_submission={ self.actions_pending_submission.clone() }
                            db={ self.db.clone() }
                            { current_tag }
                            { user_knows_current_tag }
                            tasks_open={ tasks.open }
                            tasks_done={ tasks.done }
                            tasks_backlog={ tasks.backlog }
                            on_logout={ ctx.link().callback(|_| AppMsg::Logout) }
                            on_action={ ctx.link().callback(AppMsg::NewUserAction) }
                            { on_order_change }
                        />
                    </main>
                </div>
            </div>
        }
    }
}

fn send_action(ctx: &Context<App>, a: Action) {
    let info = ctx.props().login.clone();
    ctx.link().send_future(async move {
        api::send_action(&info, a).await;
        AppMsg::ActionSubmissionComplete
    });
}
