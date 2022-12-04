use futures::{channel::oneshot, executor::block_on, FutureExt};
use gloo_storage::{LocalStorage, Storage};
use risuto_api::*;
use std::{collections::VecDeque, rc::Rc};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::{api, ui, ui::TaskOrderChangeEvent, LoginInfo};

const KEY_EVTS_PENDING_SUBMISSION: &str = "events-pending-submission";

#[derive(Clone, PartialEq, Properties)]
pub struct AppProps {
    pub login: LoginInfo,
    pub on_logout: Callback<()>,
}

pub enum AppMsg {
    Logout,
    ReceivedDb(DbDump),
    SetTag(Option<TagId>),
    NewUserEvent(NewEvent),
    NewNetworkEvent(NewEvent),
    EventSubmissionComplete,
}

pub struct App {
    client: reqwest::Client,
    db: DbDump,
    offline: bool,
    tag: Option<TagId>,
    events_pending_submission: VecDeque<NewEvent>, // push_back, pop_front
    feed_canceller: oneshot::Receiver<()>,
}

#[derive(Clone)]
struct TaskLists {
    open: Rc<Vec<(TaskId, Task)>>,
    backlog: Rc<Vec<(TaskId, Task)>>,
}

impl App {
    fn locally_insert_new_event(&mut self, e: NewEvent) {
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

    fn current_task_lists(&self) -> TaskLists {
        let tasks = self.db.tasks.iter();
        let (backlog, open) = if let Some(tag) = self.tag {
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
        TaskLists {
            open: Rc::new(open),
            backlog: Rc::new(backlog),
        }
    }
}

impl Component for App {
    type Message = AppMsg;
    type Properties = AppProps;

    fn create(ctx: &Context<Self>) -> Self {
        // Prepare basic metadata
        let client = reqwest::Client::new();

        // Connect to websocket event feed
        let feed_sender = ctx.link().clone();
        let (feed_cancel_receiver, feed_canceller) = oneshot::channel();
        spawn_local(api::start_event_feed(
            ctx.props().login.clone(),
            feed_sender,
            feed_cancel_receiver,
        ));

        // Load event submission queue
        let events_pending_submission: VecDeque<NewEvent> =
            LocalStorage::get(KEY_EVTS_PENDING_SUBMISSION).unwrap_or(VecDeque::new());

        // Start event submission if need be
        if !events_pending_submission.is_empty() {
            send_event(&client, ctx, events_pending_submission[0].clone());
        }

        // Finally, fetch a DB dump from the server
        ctx.link().send_future(
            api::fetch_db_dump(client.clone(), ctx.props().login.clone()).map(AppMsg::ReceivedDb),
        );

        App {
            client,
            db: DbDump::stub(),
            offline: true,
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
            AppMsg::ReceivedDb(db) => {
                self.db = db;
                self.offline = false;
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
                tracing::debug!("got new user event {e:?}");
                // Sanity-check that we're allowed to submit the event before adding it to the queue
                assert!(
                    block_on(e.is_authorized(&mut self.db)).expect("checking is_authorized on local db dump"),
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
                    send_event(&self.client, ctx, e.clone());
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
                    send_event(&self.client, ctx, e);
                }
            }
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let tasks = self.current_task_lists();

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
            let tasks = tasks.clone();
            ctx.link().batch_callback(move |e: TaskOrderChangeEvent| {
                let task_id = match e.before.in_backlog {
                    true => tasks.backlog[e.before.index].0,
                    false => tasks.open[e.before.index].0,
                };
                let mut insert_into = match e.after.in_backlog {
                    true => (*tasks.backlog).clone(),
                    false => (*tasks.open).clone(),
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
            <div class="container-fluid vh-100">
                <div class="row h-100">
                    <nav class="col-md-2 sidebar overflow-auto">
                        <ui::TagList
                            tags={self.db.tags.clone()}
                            current_user={self.db.owner}
                            active={self.tag}
                            on_select_tag={ctx.link().callback(|id| AppMsg::SetTag(id))}
                        />
                    </nav>
                    <main class="col-md-10 h-100">
                        <ui::MainView
                            offline={self.offline}
                            events_pending_submission={self.events_pending_submission.clone()}
                            tasks_open={tasks.open}
                            tasks_backlog={tasks.backlog}
                            on_logout={ctx.link().callback(|_| AppMsg::Logout)}
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

fn send_event(client: &reqwest::Client, ctx: &Context<App>, e: NewEvent) {
    let client = client.clone();
    let info = ctx.props().login.clone();
    ctx.link().send_future(async move {
        api::send_event(&client, &info, e).await;
        AppMsg::EventSubmissionComplete
    });
}
