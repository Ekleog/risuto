use chrono::Timelike;
use futures::{channel::oneshot, executor::block_on};
use gloo_storage::{LocalStorage, Storage};
use risuto_client::{
    api::{Event, EventData, TagId},
    DbDump, Task,
};
use std::{collections::VecDeque, rc::Rc, sync::Arc};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::{
    api, ui,
    ui::{ListType, TaskOrderChangeEvent},
    util, LoginInfo, TODAY_TAG,
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

    SetTag(Option<TagId>),
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
    tag: Option<TagId>,
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
                task.refresh_metadata();
            }
        }
    }

    fn current_task_lists(&self) -> TaskLists {
        let mut open = Vec::new();
        let mut done = Vec::new();
        let mut backlog = Vec::new();
        let now = chrono::Utc::now().with_timezone(&util::local_tz());
        // TODO: improved depending on https://github.com/chronotope/chrono/pull/927
        let end_of_today =
            now + chrono::Duration::seconds(86400 - now.num_seconds_from_midnight() as i64);
        for t in self.db.tasks.values() {
            if t.blocked_until.map(|t| t > end_of_today).unwrap_or(false) {
                continue;
            }
            if let Some(tag) = self.tag {
                if let Some(info) = t.current_tags.get(&tag) {
                    if info.backlog {
                        backlog.push((info.priority, t.clone()));
                    } else if t.is_done {
                        done.push((info.priority, t.clone()));
                    } else {
                        open.push((info.priority, t.clone()));
                    }
                }
            } else {
                if t.current_tags.len() == 0 {
                    if t.is_done {
                        done.push((0, t.clone()));
                    } else {
                        open.push((0, t.clone()));
                    }
                }
            }
        }
        open.sort_unstable_by_key(|(prio, t)| (*prio, t.id));
        done.sort_unstable_by_key(|(prio, t)| (*prio, t.id));
        backlog.sort_unstable_by_key(|(prio, t)| (*prio, t.id));
        let cleanup = |v: Vec<(i64, Arc<Task>)>| Rc::new(v.into_iter().map(|(_, t)| t).collect());
        TaskLists {
            open: cleanup(open),
            done: cleanup(done),
            backlog: cleanup(backlog),
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
            tag: Some(TagId::stub()), // A value that cannot happen when choosing an actual tag
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
                if self.tag == Some(TagId::stub()) {
                    self.tag = Some(
                        self.db
                            .tags
                            .iter()
                            .find(|(_, t)| t.0.name == TODAY_TAG)
                            .expect("found no tag named 'today'")
                            .0
                            .clone(),
                    );
                }
                self.connection_state = ConnState::Connected;
            }
            AppMsg::SetTag(id) => {
                self.tag = id;
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
            let tag = self.tag.clone();
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
                    tag.expect("attempted to reorder in untagged list"),
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
                        <ui::TagList
                            tags={self.db.tags.clone()}
                            current_user={self.db.owner}
                            active={self.tag}
                            on_select_tag={ctx.link().callback(|id| AppMsg::SetTag(id))}
                        />
                    </nav>
                    <main class="col-md-10 h-100 p-0">
                        <ui::MainView
                            connection_state={self.connection_state.clone()}
                            events_pending_submission={self.events_pending_submission.clone()}
                            db={self.db.clone()}
                            current_tag={self.tag}
                            tasks_open={tasks.open}
                            tasks_done={tasks.done}
                            tasks_backlog={tasks.backlog}
                            on_logout={ctx.link().callback(|_| AppMsg::Logout)}
                            on_event={ctx.link().callback(AppMsg::NewUserEvent)}
                            {on_order_change}
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
