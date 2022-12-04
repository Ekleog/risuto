use std::collections::HashMap;

use chrono::Duration;

const NUM_USERS: usize = 3;

const NUM_TAGS: usize = 15;
const NUM_PERMS: usize = 30;

const NUM_TASKS: usize = 150;
const TASK_TITLE_LEN: i64 = 10;

const NUM_EVENTS_PER_TYPE: usize = 300;
const COMMENT_PARAGRAPH_COUNT: i64 = 2;
const COMMENT_SENTENCE_COUNT: i64 = 3;
const COMMENT_WORD_COUNT: i64 = 10;

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

fn gen_task_title() -> String {
    mockd::words::sentence(TASK_TITLE_LEN)
}

fn gen_comment_text() -> String {
    mockd::words::paragraph(
        COMMENT_PARAGRAPH_COUNT,
        COMMENT_SENTENCE_COUNT,
        COMMENT_WORD_COUNT,
        String::from("\n"),
    )
}

fn gen_bool() -> bool {
    // mockd's bool generation is borken https://github.com/jerusdp/mockd/pull/178
    simplerand::randn(2) == 0
}

fn main() {
    // Generate users
    let mut users = Vec::new();
    gen_n_items("users", NUM_USERS, |_| {
        let uuid = mockd::unique::uuid_v4();
        users.push(uuid.clone());
        format!(
            "('{}', '{}', '{}')",
            uuid,
            mockd::internet::username(),
            mockd::password::generate(true, true, true, 6),
        )
    });
    let gen_user = || -> String { users[simplerand::randn(users.len())].clone() };

    // Generate tags
    let mut tags = Vec::new();
    gen_n_items("tags", NUM_TAGS, |i| {
        let uuid = mockd::unique::uuid_v4();
        tags.push(uuid.clone());
        // first, generate the "today" tags, then generate other tags
        let (user, tag) = match i < NUM_USERS {
            true => (users[i].clone(), String::from("today")),
            false => (gen_user(), mockd::words::word()),
        };
        format!("('{}', '{}', '{}', {})", uuid, user, tag, gen_bool())
    });
    let gen_tag = || -> String { tags[simplerand::randn(tags.len())].clone() };

    // Generate permissions
    gen_n_items("perms", NUM_PERMS, |_| {
        format!(
            "('{}', '{}', {}, {}, {}, {})",
            gen_tag(),
            gen_user(),
            gen_bool(),
            gen_bool(),
            gen_bool(),
            gen_bool()
        )
    });

    // Generate tasks
    let mut tasks = Vec::new();
    gen_n_items("tasks", NUM_TASKS, |_| {
        let uuid = mockd::unique::uuid_v4();
        tasks.push(uuid.clone());
        format!(
            "('{}', '{}', '{}', '{}')",
            uuid,
            gen_user(),
            mockd::datetime::date(),
            gen_task_title(),
        )
    });
    let gen_task = || -> String { tasks[simplerand::randn(tasks.len())].clone() };

    // First generate add_comment_events, as they need special handling
    let mut comments = HashMap::new();
    gen_n_items("add_comment_events", NUM_EVENTS_PER_TYPE, |_| {
        let uuid = mockd::unique::uuid_v4();
        let date = mockd::datetime::date();
        comments.insert(uuid.clone(), date.clone());
        format!(
            "('{}', '{}', '{}', '{}', '{}')",
            uuid,
            gen_user(),
            date,
            gen_task(),
            gen_comment_text(),
        )
    });
    let gen_comment = || -> String {
        comments
            .keys()
            .skip(simplerand::randn(comments.len()))
            .next()
            .unwrap()
            .clone()
    };

    // Helper macros
    macro_rules! evt_gen {
        ( $table:expr, $($t:tt)* ) => {
            gen_n_items($table, NUM_EVENTS_PER_TYPE, |_| {
                format!(
                    "('{}', '{}', '{}', {})",
                    mockd::unique::uuid_v4(),
                    gen_user(),
                    mockd::datetime::date(),
                    format!($($t)*),
                )
            })
        }
    }
    macro_rules! comm_evt_gen {
        ( $table:expr, $($t:tt)* ) => {
            gen_n_items($table, NUM_EVENTS_PER_TYPE, |_| {
                let comm = gen_comment();
                let date = comments.get(&comm).unwrap();
                let offset = Duration::milliseconds(simplerand::randn(i64::MAX));
                let second = Duration::milliseconds(1);
                let date = date.checked_add_signed(offset).unwrap_or(date.checked_add_signed(second).unwrap());
                format!(
                    "('{}', '{}', '{}', '{}', {})",
                    mockd::unique::uuid_v4(),
                    gen_user(),
                    date,
                    comm,
                    format!($($t)*),
                )
            })
        }
    }

    // Generate events
    evt_gen!(
        "set_title_events",
        "'{}', '{}'",
        gen_task(),
        gen_task_title(),
    );
    evt_gen!("set_task_done_events", "'{}', {}", gen_task(), gen_bool(),);
    evt_gen!(
        "set_task_archived_events",
        "'{}', {}",
        gen_task(),
        gen_bool(),
    );
    evt_gen!(
        "schedule_events",
        "'{}', {}",
        gen_task(),
        match gen_bool() {
            true => format!("'{}'", mockd::datetime::date()),
            false => String::from("NULL"),
        },
    );
    evt_gen!(
        "add_dependency_events",
        "'{}', '{}'",
        gen_task(),
        gen_task(),
    );
    evt_gen!(
        "remove_dependency_events",
        "'{}', '{}'",
        gen_task(),
        gen_task(),
    );
    evt_gen!(
        "add_tag_events",
        "'{}', '{}', {}, {}",
        gen_task(),
        gen_tag(),
        simplerand::rand::<i64>(),
        gen_bool(),
    );
    evt_gen!("remove_tag_events", "'{}', '{}'", gen_task(), gen_tag());
    comm_evt_gen!("edit_comment_events", "'{}'", gen_comment_text(),);
    comm_evt_gen!("set_comment_read_events", "'{}'", gen_bool(),);
}
