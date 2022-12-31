use futures::{channel::oneshot, executor::block_on};
use gloo_storage::{LocalStorage, Storage};
use risuto_client::{
    api::{Event, EventData},
    DbDump, Order, Search, Task,
};
use std::{collections::VecDeque, rc::Rc, sync::Arc};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::{
    api, ui,
    ui::{ListType, TaskOrderChangeEvent},
    util, LoginInfo,
};

const KEY_EVTS_PENDING_SUBMISSION: &str = "events-pending-submission";

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

    SetActiveSearch(usize),
    NewUserEvent(Event),
    NewNetworkEvent(Event),
    EventSubmissionComplete,
}

#[derive(Clone, PartialEq)]
pub enum ConnState {
    Disconnected,
    WebsocketConnected(VecDeque<Event>),
    Connected,
}

pub struct App {
    db: Rc<DbDump>,
    connection_state: ConnState,
    searches: Vec<Search>,
    active_search: usize,
    events_pending_submission: VecDeque<Event>, // push_back, pop_front
    feed_canceller: oneshot::Receiver<()>,
}

#[derive(Clone)]
struct TaskLists {
    open: Rc<Vec<Arc<Task>>>,
    done: Rc<Vec<Arc<Task>>>,
    backlog: Rc<Vec<Arc<Task>>>,
}

impl App {
    fn locally_insert_new_event(&mut self, e: Event) {
        let db = Rc::make_mut(&mut self.db);
        match db.tasks.get_mut(&e.task_id) {
            None => tracing::warn!(evt=?e, "got event for task not in db"),
            Some(t) => {
                let task = Arc::make_mut(t);
                task.add_event(e);
                task.refresh_metadata(&db.owner);
            }
        }
    }

    fn current_task_lists(&self) -> TaskLists {
        let search = &self.searches[self.active_search];
        let mut all_tasks = self.db.search(search);
        match search.order {
            Order::Tag(tag) => {
                let backlog = Rc::new(all_tasks.split_off(
                    all_tasks.partition_point(|t| !t.current_tags.get(&tag).unwrap().backlog),
                ));
                let done = Rc::new(all_tasks.split_off(all_tasks.partition_point(|t| !t.is_done)));
                let open = Rc::new(all_tasks);
                TaskLists {
                    open,
                    done,
                    backlog,
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
        let events_pending_submission: VecDeque<Event> =
            LocalStorage::get(KEY_EVTS_PENDING_SUBMISSION).unwrap_or(VecDeque::new());

        // Start event submission if need be
        if !events_pending_submission.is_empty() {
            send_event(ctx, events_pending_submission[0].clone());
        }

        App {
            db: Rc::new(DbDump::stub()),
            connection_state: ConnState::Disconnected,
            searches: vec![Search::untagged()],
            active_search: 0,
            events_pending_submission,
            feed_canceller,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            AppMsg::Logout => {
                self.feed_canceller.close(); // This should be unneeded as it closes on drop, but better safe than sorry
                LocalStorage::delete(KEY_EVTS_PENDING_SUBMISSION);
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
                for e in self.events_pending_submission.clone() {
                    self.locally_insert_new_event(e.clone());
                }
                let events_already_received = match &self.connection_state {
                    ConnState::WebsocketConnected(e) => e.clone(),
                    _ => panic!("received database while websocket is not connected"),
                };
                for e in events_already_received {
                    self.locally_insert_new_event(e);
                }
                self.searches = self
                    .db
                    .tags
                    .values()
                    .map(|(t, _)| Search::for_tag(t))
                    .collect();
                util::sort_tags(&self.db.owner, &mut self.searches, |s| {
                    &self.db.tags.get(&s.is_order_tag().unwrap()).unwrap().0
                });
                self.searches.push(Search::untagged());
                if self.active_search >= self.searches.len() {
                    self.active_search = 0;
                }
                self.connection_state = ConnState::Connected;
            }
            AppMsg::SetActiveSearch(id) => {
                self.active_search = id;
            }
            AppMsg::NewUserEvent(e) => {
                tracing::debug!("got new user event {e:?}");
                // Sanity-check that we're allowed to submit the event before adding it to the queue
                assert!(
                    block_on(e.is_authorized(&mut &*self.db)).expect("checking is_authorized on local db dump"),
                    "Submitted userevent that is not authorized. The button should have been disabled! {e:?}",
                );
                tracing::trace!("user event authorized {e:?}");

                // Submit the event to the upload queue and update our state
                self.events_pending_submission.push_back(e.clone());
                LocalStorage::set(KEY_EVTS_PENDING_SUBMISSION, &self.events_pending_submission)
                    .expect("failed saving queue to local storage");
                tracing::trace!("events pending submission queue saved");
                if self.events_pending_submission.len() == 1 {
                    // this is the first event from the queue
                    send_event(ctx, e.clone());
                    tracing::debug!("started event submission with event {e:?}");
                }
                self.locally_insert_new_event(e.clone());
                tracing::debug!("handled new user event {e:?}");
            }
            AppMsg::NewNetworkEvent(e) => self.locally_insert_new_event(e),
            AppMsg::EventSubmissionComplete => {
                self.events_pending_submission.pop_front();
                LocalStorage::set(KEY_EVTS_PENDING_SUBMISSION, &self.events_pending_submission)
                    .expect("failed saving queue to local storage");
                if !self.events_pending_submission.is_empty() {
                    let e = self.events_pending_submission[0].clone();
                    send_event(ctx, e);
                }
            }
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let tasks = self.current_task_lists();

        let on_order_change = {
            let owner = self.db.owner.clone();
            let search = self.searches[self.active_search].clone();
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
                    .map(|e| AppMsg::NewUserEvent(e))
                    .collect::<Vec<_>>();
                if e.before.list.is_done() != e.after.list.is_done() {
                    evts.push(AppMsg::NewUserEvent(Event::now(
                        owner,
                        task_id,
                        EventData::SetDone(e.after.list.is_done()),
                    )));
                }
                evts
            })
        };

        html! {
            <div class="container-fluid vh-100">
                <div class="row h-100">
                    <nav class="col-md-2 sidebar overflow-auto p-0">
                        <ui::SearchList
                            searches={ self.searches.clone() }
                            current_user={ self.db.owner }
                            active_search={ self.active_search }
                            on_select_search={ ctx.link().callback(AppMsg::SetActiveSearch) }
                        />
                    </nav>
                    <main class="col-md-10 h-100 p-0">
                        <ui::MainView
                            connection_state={ self.connection_state.clone() }
                            events_pending_submission={ self.events_pending_submission.clone() }
                            db={ self.db.clone() }
                            current_tag={ self.searches[self.active_search].is_order_tag() }
                            tasks_open={ tasks.open }
                            tasks_done={ tasks.done }
                            tasks_backlog={ tasks.backlog }
                            on_logout={ ctx.link().callback(|_| AppMsg::Logout) }
                            on_event={ ctx.link().callback(AppMsg::NewUserEvent) }
                            { on_order_change }
                        />
                    </main>
                </div>
            </div>
        }
    }
}

fn send_event(ctx: &Context<App>, e: Event) {
    let info = ctx.props().login.clone();
    ctx.link().send_future(async move {
        api::send_event(&info, e).await;
        AppMsg::EventSubmissionComplete
    });
}
