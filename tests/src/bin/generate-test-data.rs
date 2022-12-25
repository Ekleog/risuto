use std::cell::RefCell;

use chrono::Duration;
use lipsum::lipsum_words_from_seed;
use rand::{rngs::StdRng, seq::SliceRandom, Rng, SeedableRng};

const NUM_USERS: usize = 3;

const NUM_TAGS: usize = 10;
const NUM_PERMS: usize = 20;

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

fn gen_date(rng: &mut StdRng) -> chrono::DateTime<chrono::Utc> {
    let start = chrono::DateTime::parse_from_rfc3339("1970-01-01T01:01:01Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let end = chrono::DateTime::parse_from_rfc3339("2022-01-01T01:01:01Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let nanos = end
        .signed_duration_since(start)
        .to_std()
        .unwrap()
        .as_nanos();
    let nanos = rng.gen_range(0..nanos);
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

fn gen_task_title(rng: &mut StdRng) -> String {
    escape(lipsum_words_from_seed(TASK_TITLE_LEN, rng.gen()))
}

fn gen_comment_text(rng: &mut StdRng) -> String {
    escape(lipsum_words_from_seed(COMMENT_WORD_COUNT, rng.gen()))
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
    gen_n_items("tags", NUM_TAGS, |i| {
        let uuid = gen_uuid(&mut rng);
        tags.push(uuid.clone());
        // first, generate the "today" tags, then generate other tags
        let (user, tag) = match i < NUM_USERS {
            true => (users[i].clone(), String::from("today")),
            false => (gen_user(&mut rng), gen_tag(&mut rng)),
        };
        format!("('{}', '{}', '{}', {})", uuid, user, tag, rng.gen::<bool>())
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

    // Generate tasks
    let mut tasks = Vec::new();
    gen_n_items("tasks", NUM_TASKS, |_| {
        let uuid = gen_uuid(&mut rng);
        tasks.push(uuid.clone());
        format!(
            "('{}', '{}', '{}', '{}')",
            uuid,
            gen_user(&mut rng),
            gen_date(&mut rng),
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
        let date = RefCell::new(gen_date(&mut rng));
        let task_id = RefCell::new(gen_task(&mut rng));
        let mut title = "NULL".to_string();
        let mut new_val_bool = "NULL";
        let mut time = "NULL".to_string();
        let mut tag_id = "NULL".to_string();
        let mut new_val_int = "NULL".to_string();
        let mut comment = "NULL".to_string();
        let mut parent_id = "NULL".to_string();
        let mut mk_title = |rng: &mut StdRng| title = format!("'{}'", gen_task_title(rng));
        let mut mk_bool =
            |rng: &mut StdRng| new_val_bool = if rng.gen() { "true" } else { "false" };
        let mut mk_time_maybe = |rng: &mut StdRng| {
            if rng.gen() { time = format!("'{}'", gen_date(rng)); }
        };
        let mut mk_tag = |rng: &mut StdRng| tag_id = format!("'{}'", gen_tag(rng));
        let mut mk_comment = |rng: &mut StdRng| comment = format!("'{}'", gen_comment_text(rng));
        let mut mk_parent =
            |rng: &mut StdRng, comments: &Vec<(String, String, chrono::DateTime<chrono::Utc>)>| {
                let (par_id, par_task, par_date) = comments.choose(&mut *rng).unwrap().clone();
                parent_id = format!("'{}'", par_id);
                *task_id.borrow_mut() = par_task.clone();
                let offset = Duration::milliseconds(rng.gen_range(0..1_000_000_000));
                let millis = Duration::milliseconds(1);
                let failover = date.borrow().checked_add_signed(millis).unwrap();
                *date.borrow_mut() = par_date.checked_add_signed(offset).unwrap_or(failover);
            };
        let type_ = match rng.gen_range(0..10) {
            0 => {
                mk_title(&mut rng);
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
                mk_tag(&mut rng);
                mk_bool(&mut rng);
                new_val_int = format!("{}", rng.gen::<i64>());
                "add_tag"
            }
            6 => {
                mk_tag(&mut rng);
                "remove_tag"
            }
            7 => {
                mk_comment(&mut rng);
                if !comments.is_empty() && rng.gen() {
                    mk_parent(&mut rng, &comments);
                }
                comments.push((id.clone(), task_id.borrow().clone(), *date.borrow()));
                "add_comment"
            }
            8 => {
                if comments.is_empty() {
                    continue;
                }
                mk_comment(&mut rng);
                mk_parent(&mut rng, &comments);
                "edit_comment"
            }
            9 => {
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
        print!("    ('{id}', '{owner_id}', '{date}', '{type_}', '{task_id}', {title}, {new_val_bool}, {time}, {tag_id}, {new_val_int}, {comment}, {parent_id})");
        generated += 1;
        if generated < NUM_EVENTS {
            println!(",");
        } else {
            println!();
        }
    }
    println!("ON CONFLICT DO NOTHING;");
}
