CREATE TABLE tasks (
    id UUID PRIMARY KEY NOT NULL,
    owner_id UUID NOT NULL,
    date TIMESTAMP NOT NULL,

    initial_title VARCHAR NOT NULL,

    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE
);

CREATE TYPE event_type AS ENUM (
    'set_title',
    'set_done',
    'set_archived',
    'blocked_until',
    'schedule_for',
    'add_tag',
    'remove_tag',
    'add_comment',
    'edit_comment',
    'set_comment_read'
);

CREATE TABLE events (
    -- mandatory part of the table
    id UUID PRIMARY KEY NOT NULL,
    owner_id UUID NOT NULL,
    date TIMESTAMP NOT NULL,

    type event_type NOT NULL,
    task_id UUID NOT NULL,

    -- optional part of the table, depends on the event type, checked (and documented) by `event_is_valid` constraint
    title VARCHAR,
    new_val_bool BOOLEAN,
    time TIMESTAMP,
    tag_id UUID,
    new_val_int BIGINT,
    comment TEXT,
    parent_id UUID,

    -- foreign keys
    FOREIGN KEY (owner_id) REFERENCES users (id)
        ON DELETE CASCADE,
    FOREIGN KEY (task_id) REFERENCES tasks (id)
        ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags (id)
        ON DELETE CASCADE,
    UNIQUE (id, task_id), -- needed for foreign key below, id is pkey anyway
    FOREIGN KEY (parent_id, task_id) REFERENCES events (id, task_id)
        ON DELETE CASCADE,

    -- the big constraint
    CONSTRAINT event_is_valid CHECK (
        (type = 'set_title' AND
            title IS NOT NULL AND -- the new title
            new_val_bool IS NULL AND time IS NULL AND tag_id IS NULL AND new_val_int IS NULL AND comment IS NULL AND parent_id IS NULL) OR
        ((type = 'set_done' OR type = 'set_archived') AND
            new_val_bool IS NOT NULL AND -- the new state
            title IS NULL AND time IS NULL AND tag_id IS NULL AND new_val_int IS NULL AND comment IS NULL AND parent_id IS NULL) OR
        ((type = 'blocked_until' OR type = 'schedule_for') AND
            time IS NOT NULL AND -- date at which the task state will change
            title IS NULL AND new_val_bool IS NULL AND tag_id IS NULL AND new_val_int IS NULL AND comment IS NULL AND parent_id IS NULL) OR
        (type = 'add_tag' AND
            new_val_bool IS NOT NULL AND -- whether the task is in this tag's backlog
            tag_id IS NOT NULL AND -- the tag added
            new_val_int IS NOT NULL AND -- the priority of the task within this tag (lower is higher in the list)
            title IS NULL AND time IS NULL AND comment IS NULL AND parent_id IS NULL) OR
        (type = 'remove_tag' AND
            tag_id IS NOT NULL AND -- the tag removed
            title IS NULL AND new_val_bool IS NULL AND time IS NULL AND new_val_int IS NULL AND comment IS NULL AND parent_id IS NULL) OR
        (type = 'add_comment' AND
            comment IS NOT NULL AND -- comment text
            -- parent_id can be either null or not-null depending on whether the comment is a reply to another comment
            title IS NULL AND new_val_bool IS NULL AND time IS NULL AND tag_id IS NULL AND new_val_int IS NULL) OR
        (type = 'edit_comment' AND
            comment IS NOT NULL AND -- comment text
            parent_id IS NOT NULL AND -- edited comment
            title IS NULL AND new_val_bool IS NULL AND time IS NULL AND tag_id IS NULL AND new_val_int IS NULL) OR
        (type = 'set_comment_read' AND
            parent_id IS NOT NULL AND -- the comment or comment edit marked as (un)read
            new_val_bool IS NOT NULL AND -- true iff the comment (edit) is now read
            title IS NULL AND time IS NULL AND tag_id IS NULL AND new_val_int IS NULL AND comment IS NULL)
    )
);
