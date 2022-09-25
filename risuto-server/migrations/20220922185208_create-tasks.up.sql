PRAGMA foreign_keys = ON;

CREATE TABLE tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    owner_id INTEGER NOT NULL,
    date TIMESTAMP NOT NULL,

    title VARCHAR NOT NULL,
    scheduled TIMESTAMP NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE
);

CREATE TABLE task_dependencies_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    owner_id INTEGER NOT NULL,
    date TIMESTAMP NOT NULL,

    first_id INTEGER NOT NULL,
    then_id INTEGER NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (first_id) REFERENCES tasks (id)
        ON DELETE CASCADE,
    FOREIGN KEY (then_id) REFERENCES tasks (id)
        ON DELETE CASCADE
);

CREATE TABLE tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    owner_id INTEGER NOT NULL,

    name VARCHAR NOT NULL UNIQUE,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE
);

CREATE TABLE add_tag_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    owner_id INTEGER NOT NULL,
    date TIMESTAMP NOT NULL,

    task_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (task_id) REFERENCES tasks (id)
        ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags (id)
        ON DELETE CASCADE
);

CREATE TABLE add_comment_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    owner_id INTEGER NOT NULL,
    date TIMESTAMP NOT NULL,

    task_id INTEGER NOT NULL,
    text TEXT NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (task_id) REFERENCES tasks (id)
        ON DELETE CASCADE
);

CREATE TABLE edit_comment_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    owner_id INTEGER NOT NULL,
    date TIMESTAMP NOT NULL,

    task_id INTEGER NOT NULL,
    comment_id INTEGER NOT NULL,
    text TEXT NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (task_id) REFERENCES tasks (id)
        ON DELETE CASCADE,
    FOREIGN KEY (comment_id) REFERENCES add_comment_events (id)
        ON DELETE CASCADE
);
