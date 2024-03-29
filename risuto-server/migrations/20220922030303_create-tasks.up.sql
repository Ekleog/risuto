CREATE TABLE tasks (
    id UUID PRIMARY KEY NOT NULL,
    owner_id UUID NOT NULL,
    date TIMESTAMP NOT NULL,

    initial_title VARCHAR NOT NULL,
    top_comment_id UUID NOT NULL, -- see down this file for the fkey constraint

    FOREIGN KEY (owner_id) REFERENCES users (id)
);

CREATE TYPE event_type AS ENUM (
    'set_title',
    'set_done',
    'set_archived',
    'blocked_until',
    'schedule_for',
    'set_order',
    'add_tag',
    'remove_tag',
    'add_comment',
    'edit_comment',
    'set_event_read'
);

CREATE TABLE events (
    -- mandatory part of the table
    id UUID PRIMARY KEY NOT NULL,
    owner_id UUID NOT NULL,
    date TIMESTAMP NOT NULL,
    task_id UUID NOT NULL,

    d_type event_type NOT NULL,
    -- optional part of the table, depends on the event type, checked (and documented) by `event_is_valid` constraint
    d_text TEXT,
    d_bool BOOLEAN,
    d_int BIGINT,
    d_time TIMESTAMP,
    d_tag_id UUID,
    d_parent_id UUID,
    d_order_id UUID,

    -- foreign keys
    FOREIGN KEY (owner_id) REFERENCES users (id),
    FOREIGN KEY (task_id) REFERENCES tasks (id)
        ON DELETE CASCADE,
    FOREIGN KEY (d_tag_id) REFERENCES tags (id)
        ON DELETE CASCADE,
    UNIQUE (id, task_id), -- needed for foreign key below, id is pkey anyway
    UNIQUE (id, owner_id, date), -- needed for foreign key at the bottom of the file, id is pkey anyway
    FOREIGN KEY (d_parent_id, task_id) REFERENCES events (id, task_id),

    -- the big constraint
    CONSTRAINT event_is_valid CHECK (
        (d_type = 'set_title' AND
            d_text IS NOT NULL AND -- the new title
            d_bool IS NULL AND d_int IS NULL AND d_time IS NULL AND d_tag_id IS NULL AND d_parent_id IS NULL AND d_order_id IS NULL) OR
        ((d_type = 'set_done' OR d_type = 'set_archived') AND
            d_bool IS NOT NULL AND -- the new state
            d_text IS NULL AND d_int IS NULL AND d_time IS NULL AND d_tag_id IS NULL AND d_parent_id IS NULL AND d_order_id IS NULL) OR
        ((d_type = 'blocked_until' OR d_type = 'schedule_for') AND
            -- time is the date at which the task state will change, can be null to unset
            d_text IS NULL AND d_bool IS NULL AND d_int IS NULL AND d_tag_id IS NULL AND d_parent_id IS NULL AND d_order_id IS NULL) OR
        (d_type = 'set_order' AND
            d_order_id IS NOT NULL AND d_int IS NOT NULL AND
            d_text IS NULL AND d_bool IS NULL AND d_time IS NULL AND d_tag_id IS NULL AND d_parent_id IS NULL) OR
        (d_type = 'add_tag' AND
            d_bool IS NOT NULL AND -- whether the task is in this tag's backlog
            d_int IS NOT NULL AND -- the priority of the task within this tag (lower is higher in the list)
            d_tag_id IS NOT NULL AND -- the tag added
            d_text IS NULL AND d_time IS NULL AND d_parent_id IS NULL AND d_order_id IS NULL) OR
        (d_type = 'remove_tag' AND
            d_tag_id IS NOT NULL AND -- the tag removed
            d_text IS NULL AND d_bool IS NULL AND d_int IS NULL AND d_time IS NULL AND d_parent_id IS NULL AND d_order_id IS NULL) OR
        (d_type = 'add_comment' AND
            d_text IS NOT NULL AND -- comment text
            -- parent_id can be either null or not-null depending on whether the comment is a reply to another comment
            d_bool IS NULL AND d_int IS NULL AND d_time IS NULL AND d_tag_id IS NULL AND d_order_id IS NULL) OR
        (d_type = 'edit_comment' AND
            d_text IS NOT NULL AND -- comment text
            d_parent_id IS NOT NULL AND -- edited comment
            d_bool IS NULL AND d_int IS NULL AND d_time IS NULL AND d_tag_id IS NULL AND d_order_id IS NULL) OR
        (d_type = 'set_event_read' AND
            d_parent_id IS NOT NULL AND -- the comment or comment edit marked as (un)read
            d_bool IS NOT NULL AND -- true iff the comment (edit) is now read
            d_text IS NULL AND d_int IS NULL AND d_time IS NULL AND d_tag_id IS NULL AND d_order_id IS NULL)
    )
);

ALTER TABLE tasks
ADD CONSTRAINT task_has_top_comment
    FOREIGN KEY (top_comment_id, owner_id, date)
    REFERENCES events (id, owner_id, date) -- TODO: find a way to have as constraint that the event referenced is an add_comment event with no parent
    DEFERRABLE
    INITIALLY IMMEDIATE;
