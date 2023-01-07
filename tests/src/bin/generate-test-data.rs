use std::cell::RefCell;

use bolero::{bolero_engine::TypeGenerator, generator::bolero_generator::driver::ForcedRng};
use chrono::Duration;
use lipsum::lipsum_words_from_seed;
use rand::{rngs::StdRng, seq::SliceRandom, Rng, SeedableRng};
use risuto_api::Query;
use risuto_client::{OrderType, Order};

const NUM_USERS: usize = 3;

const NUM_TAGS: usize = 10;
const NUM_PERMS: usize = 20;
const NUM_SEARCHES: usize = 20;

const NUM_TASKS: usize = 200;
const TASK_TITLE_LEN: usize = 10;

const NUM_EVENTS: usize = 5000;
const COMMENT_WORD_COUNT: usize = 10;

fn gen_n_items(table: &str, n: usize, mut f: impl FnMut(usize) -> String) {
    println!("INSERT INTO {} VALUES", table);
    for i in 0..n {
        if i != 0 {
            println!(",");
        }
        print!("    {}", f(i));
    }
    println!();
    println!("ON CONFLICT DO NOTHING;");
}

fn escape(s: String) -> String {
    s.chars().filter(|&c| c != '\'').collect()
}

fn gen_uuid(rng: &mut StdRng) -> String {
    format!(
        "{}",
        uuid::Builder::from_random_bytes(rng.gen()).into_uuid()
    )
}

fn gen_username(rng: &mut StdRng) -> String {
    let mut res = lipsum_words_from_seed(1, rng.gen());
    res.pop(); // remove the ending '.'
    escape(res)
}

fn gen_password(rng: &mut StdRng) -> String {
    escape(lipsum_words_from_seed(1, rng.gen()))
}

fn gen_tag(rng: &mut StdRng) -> String {
    let mut res = lipsum_words_from_seed(1, rng.gen());
    res.pop(); // remove the ending '.'
    escape(res)
}

fn gen_search_name(rng: &mut StdRng) -> String {
    gen_tag(rng)
}

fn gen_date_range(rng: &mut StdRng, start: &str, end: &str) -> chrono::DateTime<chrono::Utc> {
    let start = chrono::DateTime::parse_from_rfc3339(start)
        .unwrap()
        .with_timezone(&chrono::Utc);
    let end = chrono::DateTime::parse_from_rfc3339(end)
        .unwrap()
        .with_timezone(&chrono::Utc);
    let max_nanos = end
        .signed_duration_since(start)
        .to_std()
        .unwrap()
        .as_nanos();
    let nanos = rng.gen_range(0..max_nanos);
    start
        .checked_add_signed(
            chrono::Duration::from_std(
                std::time::Duration::from_secs((nanos / 1_000_000_000) as u64)
                    .checked_add(std::time::Duration::from_nanos(
                        (nanos % 1_000_000_000) as u64,
                    ))
                    .unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
}

fn gen_date(rng: &mut StdRng) -> chrono::DateTime<chrono::Utc> {
    gen_date_range(rng, "2020-01-01T01:01:01Z", "2030-01-01T01:01:01Z")
}

fn gen_past_date(rng: &mut StdRng) -> chrono::DateTime<chrono::Utc> {
    gen_date_range(rng, "2010-01-01T01:01:01Z", "2020-01-01T01:01:01Z")
}

fn gen_task_title(rng: &mut StdRng) -> String {
    escape(lipsum_words_from_seed(TASK_TITLE_LEN, rng.gen()))
}

fn gen_comment_text(rng: &mut StdRng) -> String {
    escape(lipsum_words_from_seed(COMMENT_WORD_COUNT, rng.gen()))
}

fn gen_bolero<T: TypeGenerator>(rng: &mut StdRng) -> T {
    T::generate(&mut ForcedRng::new(rng)).unwrap()
}

fn main() {
    // Prepare RNG
    let mut rng = rand::rngs::StdRng::from_seed(Default::default());

    // Generate users
    let mut users = Vec::new();
    gen_n_items("users", NUM_USERS, |_| {
        let uuid = gen_uuid(&mut rng);
        users.push(uuid.clone());
        format!(
            "('{}', '{}', '{}')",
            uuid,
            gen_username(&mut rng),
            gen_password(&mut rng),
        )
    });
    let gen_user = |rng: &mut StdRng| -> String { users.choose(rng).unwrap().clone() };

    // Generate tags
    let mut tags = Vec::new();
    gen_n_items("tags", NUM_TAGS, |_| {
        let uuid = gen_uuid(&mut rng);
        tags.push(uuid.clone());
        let user = gen_user(&mut rng);
        let tag = gen_tag(&mut rng);
        let archived = rng.gen::<bool>();
        format!("('{uuid}', '{user}', '{tag}', {archived})")
    });
    let gen_tag = |rng: &mut StdRng| -> String { tags.choose(rng).unwrap().clone() };

    // Generate permissions
    gen_n_items("perms", NUM_PERMS, |_| {
        format!(
            "('{}', '{}', {}, {}, {}, {})",
            gen_tag(&mut rng),
            gen_user(&mut rng),
            rng.gen::<bool>(),
            rng.gen::<bool>(),
            rng.gen::<bool>(),
            rng.gen::<bool>(),
        )
    });

    // Generate searches
    gen_n_items("searches", NUM_SEARCHES, |_| {
        let id = gen_uuid(&mut rng);
        let name = gen_search_name(&mut rng);
        let filter = serde_json::to_string(&gen_bolero::<Query>(&mut rng)).unwrap();
        let order_type = gen_bolero::<Order>(&mut rng);
        let tag = match order_type {
            Order::Tag(_) => {
                format!("'{}'", gen_tag(&mut rng)) // ignore the given tag as it doesn't respect fkeys
            }
            _ => String::from("NULL"),
        };
        let order_type = match order_type {
            Order::Custom(_) => "custom",
            Order::Tag(_) => "tag",
            Order::CreationDate(OrderType::Asc) => "creation_date_asc",
            Order::CreationDate(OrderType::Desc) => "creation_date_desc",
            Order::LastEventDate(OrderType::Asc) => "last_event_date_asc",
            Order::LastEventDate(OrderType::Desc) => "last_event_date_desc",
            Order::ScheduledFor(OrderType::Asc) => "scheduled_for_asc",
            Order::ScheduledFor(OrderType::Desc) => "scheduled_for_desc",
            Order::BlockedUntil(OrderType::Asc) => "blocked_until_asc",
            Order::BlockedUntil(OrderType::Desc) => "blocked_until_desc",
        };
        format!("('{id}', '{name}', '{filter}', '{order_type}', {tag})")
    });

    // Generate tasks
    let mut tasks = Vec::new();
    gen_n_items("tasks", NUM_TASKS, |_| {
        let uuid = gen_uuid(&mut rng);
        tasks.push(uuid.clone());
        format!(
            "('{}', '{}', '{}', '{}')",
            uuid,
            gen_user(&mut rng),
            gen_past_date(&mut rng),
            gen_task_title(&mut rng),
        )
    });
    let gen_task = |rng: &mut StdRng| -> String { tasks.choose(rng).unwrap().clone() };

    // Finally, generate events
    let mut comments = Vec::new();
    let mut generated = 0;
    println!("INSERT INTO events VALUES");
    while generated < NUM_EVENTS {
        let id = gen_uuid(&mut rng);
        let owner_id = gen_user(&mut rng);
        let date = RefCell::new(gen_past_date(&mut rng));
        let task_id = RefCell::new(gen_task(&mut rng));
        let mut d_text = "NULL".to_string();
        let mut d_bool = "NULL";
        let mut d_int = "NULL".to_string();
        let mut d_time = "NULL".to_string();
        let mut d_tag_id = "NULL".to_string();
        let mut d_parent_id = "NULL".to_string();
        let mut d_order_id = "NULL".to_string();
        let mut mk_text = |rng: &mut StdRng, is_title| {
            d_text = format!(
                "'{}'",
                if is_title {
                    gen_task_title(rng)
                } else {
                    gen_comment_text(rng)
                }
            )
        };
        let mut mk_bool = |rng: &mut StdRng| d_bool = if rng.gen() { "true" } else { "false" };
        let mut mk_int = |rng: &mut StdRng| d_int = format!("{}", rng.gen::<i64>());
        let mut mk_time_maybe = |rng: &mut StdRng| {
            if rng.gen() {
                d_time = format!("'{}'", gen_date(rng));
            }
        };
        let mut mk_tag = |rng: &mut StdRng| d_tag_id = format!("'{}'", gen_tag(rng));
        let mut mk_parent =
            |rng: &mut StdRng, comments: &Vec<(String, String, chrono::DateTime<chrono::Utc>)>| {
                let (par_id, par_task, par_date) = comments.choose(&mut *rng).unwrap().clone();
                d_parent_id = format!("'{}'", par_id);
                *task_id.borrow_mut() = par_task.clone();
                let offset = Duration::milliseconds(rng.gen_range(0..1_000_000_000));
                let millis = Duration::milliseconds(1);
                let failover = date.borrow().checked_add_signed(millis).unwrap();
                *date.borrow_mut() = par_date.checked_add_signed(offset).unwrap_or(failover);
            };
        let mut mk_order = |rng: &mut StdRng| d_order_id = format!("'{}'", gen_uuid(rng));
        let d_type = match rng.gen_range(0..11) { // TODO: replace with gen_bolero::<DbEventType>
            0 => {
                mk_text(&mut rng, true);
                "set_title"
            }
            1 => {
                mk_bool(&mut rng);
                "set_done"
            }
            2 => {
                mk_bool(&mut rng);
                "set_archived"
            }
            3 => {
                mk_time_maybe(&mut rng);
                "blocked_until"
            }
            4 => {
                mk_time_maybe(&mut rng);
                "schedule_for"
            }
            5 => {
                mk_int(&mut rng);
                mk_order(&mut rng);
                "set_order"
            }
            6 => {
                mk_tag(&mut rng);
                mk_bool(&mut rng);
                mk_int(&mut rng);
                "add_tag"
            }
            7 => {
                mk_tag(&mut rng);
                "remove_tag"
            }
            8 => {
                mk_text(&mut rng, false);
                if !comments.is_empty() && rng.gen() {
                    mk_parent(&mut rng, &comments);
                }
                comments.push((id.clone(), task_id.borrow().clone(), *date.borrow()));
                "add_comment"
            }
            9 => {
                if comments.is_empty() {
                    continue;
                }
                mk_text(&mut rng, false);
                mk_parent(&mut rng, &comments);
                "edit_comment"
            }
            10 => {
                if comments.is_empty() {
                    continue;
                }
                mk_bool(&mut rng);
                mk_parent(&mut rng, &comments);
                "set_event_read"
            }
            _ => panic!(),
        };
        let date = *date.borrow();
        let task_id = task_id.borrow().clone();
        print!("    ('{id}', '{owner_id}', '{date}', '{task_id}', '{d_type}', {d_text}, {d_bool}, {d_int}, {d_time}, {d_tag_id}, {d_parent_id}, {d_order_id})");
        generated += 1;
        if generated < NUM_EVENTS {
            println!(",");
        } else {
            println!();
        }
    }
    println!("ON CONFLICT DO NOTHING;");
}
