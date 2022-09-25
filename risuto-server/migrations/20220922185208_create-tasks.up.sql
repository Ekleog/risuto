PRAGMA foreign_keys = ON;

CREATE TABLE tasks (
    id VARCHAR PRIMARY KEY NOT NULL,
    owner_id INTEGER NOT NULL,
    date TIMESTAMP NOT NULL,

    title VARCHAR NOT NULL,
    scheduled TIMESTAMP NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE
);

CREATE TABLE task_dependencies_events (
    id VARCHAR PRIMARY KEY NOT NULL,
    owner_id INTEGER NOT NULL,
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

CREATE TABLE tags (
    id VARCHAR PRIMARY KEY NOT NULL,
    owner_id INTEGER NOT NULL,

    name VARCHAR NOT NULL UNIQUE,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE
);

CREATE TABLE add_tag_events (
    id VARCHAR PRIMARY KEY NOT NULL,
    owner_id INTEGER NOT NULL,
    date TIMESTAMP NOT NULL,

    task_id VARCHAR NOT NULL,
    tag_id VARCHAR NOT NULL,
    priority INTEGER NOT NULL,

    UNIQUE (tag_id, priority),

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (task_id) REFERENCES tasks (id)
        ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags (id)
        ON DELETE CASCADE
);

CREATE TABLE add_comment_events (
    id VARCHAR PRIMARY KEY NOT NULL,
    owner_id INTEGER NOT NULL,
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
    owner_id INTEGER NOT NULL,
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
