CREATE TABLE tasks (
    id UUID PRIMARY KEY NOT NULL,
    owner_id UUID NOT NULL,
    date TIMESTAMP NOT NULL,

    initial_title VARCHAR NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE
);

CREATE TABLE set_title_events (
    id UUID PRIMARY KEY NOT NULL,
    owner_id UUID NOT NULL,
    date TIMESTAMP NOT NULL,

    task_id UUID NOT NULL,
    title VARCHAR NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (task_id) REFERENCES tasks (id)
        ON DELETE CASCADE
);

CREATE TABLE set_task_done_events (
    id UUID PRIMARY KEY NOT NULL,
    owner_id UUID NOT NULL,
    date TIMESTAMP NOT NULL,

    task_id UUID NOT NULL,
    now_done BOOLEAN NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (task_id) REFERENCES tasks (id)
        ON DELETE CASCADE
);

CREATE TABLE set_task_archived_events (
    id UUID PRIMARY KEY NOT NULL,
    owner_id UUID NOT NULL,
    date TIMESTAMP NOT NULL,

    task_id UUID NOT NULL,
    now_archived BOOLEAN NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (task_id) REFERENCES tasks (id)
        ON DELETE CASCADE
);

CREATE TABLE schedule_events (
    id UUID PRIMARY KEY NOT NULL,
    owner_id UUID NOT NULL,
    date TIMESTAMP NOT NULL,

    task_id UUID NOT NULL,
    scheduled_date TIMESTAMP, -- nullable, to remove scheduled date

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (task_id) REFERENCES tasks (id)
        ON DELETE CASCADE
);

CREATE TABLE add_dependency_events (
    id UUID PRIMARY KEY NOT NULL,
    owner_id UUID NOT NULL,
    date TIMESTAMP NOT NULL,

    first_id UUID NOT NULL,
    then_id UUID NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (first_id) REFERENCES tasks (id)
        ON DELETE CASCADE,
    FOREIGN KEY (then_id) REFERENCES tasks (id)
        ON DELETE CASCADE
);

CREATE TABLE remove_dependency_events (
    id UUID PRIMARY KEY NOT NULL,
    owner_id UUID NOT NULL,
    date TIMESTAMP NOT NULL,

    first_id UUID NOT NULL,
    then_id UUID NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (first_id) REFERENCES tasks (id)
        ON DELETE CASCADE,
    FOREIGN KEY (then_id) REFERENCES tasks (id)
        ON DELETE CASCADE
);

CREATE TABLE tags (
    id UUID PRIMARY KEY NOT NULL,
    owner_id UUID NOT NULL,

    name VARCHAR NOT NULL,
    archived BOOLEAN NOT NULL,

    UNIQUE (owner_id, name),
    CHECK (name ~ '^[a-zA-Z0-9]+$'),

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE
);

CREATE TABLE add_tag_events (
    id UUID PRIMARY KEY NOT NULL,
    owner_id UUID NOT NULL,
    date TIMESTAMP NOT NULL,

    task_id UUID NOT NULL,
    tag_id UUID NOT NULL,
    priority BIGINT NOT NULL, -- priority of this task within the tag
    backlog BOOLEAN NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (task_id) REFERENCES tasks (id)
        ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags (id)
        ON DELETE CASCADE
);

CREATE TABLE remove_tag_events (
    id UUID PRIMARY KEY NOT NULL,
    owner_id UUID NOT NULL,
    date TIMESTAMP NOT NULL,

    task_id UUID NOT NULL,
    tag_id UUID NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (task_id) REFERENCES tasks (id)
        ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags (id)
        ON DELETE CASCADE
);

CREATE TABLE add_comment_events (
    id UUID PRIMARY KEY NOT NULL,
    owner_id UUID NOT NULL,
    date TIMESTAMP NOT NULL,

    task_id UUID NOT NULL,
    text TEXT NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (task_id) REFERENCES tasks (id)
        ON DELETE CASCADE
);

CREATE TABLE edit_comment_events (
    id UUID PRIMARY KEY NOT NULL,
    owner_id UUID NOT NULL,
    date TIMESTAMP NOT NULL,

    comment_id UUID NOT NULL,
    text TEXT NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (comment_id) REFERENCES add_comment_events (id)
        ON DELETE CASCADE
);
