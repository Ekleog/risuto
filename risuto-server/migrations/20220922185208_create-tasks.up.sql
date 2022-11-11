PRAGMA foreign_keys = ON;

CREATE TABLE tasks (
    id VARCHAR PRIMARY KEY NOT NULL,
    owner_id VARCHAR NOT NULL,
    date TIMESTAMP NOT NULL,

    initial_title VARCHAR NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE
);

CREATE TABLE set_title_events (
    id VARCHAR PRIMARY KEY NOT NULL,
    owner_id VARCHAR NOT NULL,
    date TIMESTAMP NOT NULL,

    task_id VARCHAR NOT NULL,
    title VARCHAR NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (task_id) REFERENCES tasks (id)
        ON DELETE CASCADE
);

CREATE TABLE complete_task_events (
    id VARCHAR PRIMARY KEY NOT NULL,
    owner_id VARCHAR NOT NULL,
    date TIMESTAMP NOT NULL,

    task_id VARCHAR NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (task_id) REFERENCES tasks (id)
        ON DELETE CASCADE
);

CREATE TABLE reopen_task_events (
    id VARCHAR PRIMARY KEY NOT NULL,
    owner_id VARCHAR NOT NULL,
    date TIMESTAMP NOT NULL,

    task_id VARCHAR NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (task_id) REFERENCES tasks (id)
        ON DELETE CASCADE
);

CREATE TABLE archive_task_events (
    id VARCHAR PRIMARY KEY NOT NULL,
    owner_id VARCHAR NOT NULL,
    date TIMESTAMP NOT NULL,

    task_id VARCHAR NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (task_id) REFERENCES tasks (id)
        ON DELETE CASCADE
);

CREATE TABLE unarchive_task_events (
    id VARCHAR PRIMARY KEY NOT NULL,
    owner_id VARCHAR NOT NULL,
    date TIMESTAMP NOT NULL,

    task_id VARCHAR NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (task_id) REFERENCES tasks (id)
        ON DELETE CASCADE
);

CREATE TABLE schedule_events (
    id VARCHAR PRIMARY KEY NOT NULL,
    owner_id VARCHAR NOT NULL,
    date TIMESTAMP NOT NULL,

    task_id VARCHAR NOT NULL,
    scheduled_date TIMESTAMP, -- nullable, to remove scheduled date

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (task_id) REFERENCES tasks (id)
        ON DELETE CASCADE
);

CREATE TABLE add_dependency_events (
    id VARCHAR PRIMARY KEY NOT NULL,
    owner_id VARCHAR NOT NULL,
    date TIMESTAMP NOT NULL,

    first_id VARCHAR NOT NULL,
    then_id VARCHAR NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (first_id) REFERENCES tasks (id)
        ON DELETE CASCADE,
    FOREIGN KEY (then_id) REFERENCES tasks (id)
        ON DELETE CASCADE
);

CREATE TABLE remove_dependency_events (
    id VARCHAR PRIMARY KEY NOT NULL,
    owner_id VARCHAR NOT NULL,
    date TIMESTAMP NOT NULL,

    dep_id VARCHAR NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (dep_id) REFERENCES add_dependency_events (id)
        ON DELETE CASCADE
);

CREATE TABLE tags (
    id VARCHAR PRIMARY KEY NOT NULL,
    owner_id VARCHAR NOT NULL,

    name VARCHAR NOT NULL UNIQUE,
    archived BOOLEAN NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE
);

CREATE TABLE add_tag_events (
    id VARCHAR PRIMARY KEY NOT NULL,
    owner_id VARCHAR NOT NULL,
    date TIMESTAMP NOT NULL,

    task_id VARCHAR NOT NULL,
    tag_id VARCHAR NOT NULL,
    priority INTEGER NOT NULL, -- priority of this task within the tag

    UNIQUE (tag_id, priority),

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (task_id) REFERENCES tasks (id)
        ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags (id)
        ON DELETE CASCADE
);

CREATE TABLE remove_tag_events (
    id VARCHAR PRIMARY KEY NOT NULL,
    owner_id VARCHAR NOT NULL,
    date TIMESTAMP NOT NULL,

    add_tag_id VARCHAR NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (add_tag_id) REFERENCES add_tag_events (id)
        ON DELETE CASCADE
);

CREATE TABLE add_comment_events (
    id VARCHAR PRIMARY KEY NOT NULL,
    owner_id VARCHAR NOT NULL,
    date TIMESTAMP NOT NULL,

    task_id VARCHAR NOT NULL,
    text TEXT NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (task_id) REFERENCES tasks (id)
        ON DELETE CASCADE
);

CREATE TABLE edit_comment_events (
    id VARCHAR PRIMARY KEY NOT NULL,
    owner_id VARCHAR NOT NULL,
    date TIMESTAMP NOT NULL,

    task_id VARCHAR NOT NULL,
    comment_id VARCHAR NOT NULL,
    text TEXT NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (task_id) REFERENCES tasks (id)
        ON DELETE CASCADE,
    FOREIGN KEY (comment_id) REFERENCES add_comment_events (id)
        ON DELETE CASCADE
);
