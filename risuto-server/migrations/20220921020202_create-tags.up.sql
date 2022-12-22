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

CREATE TABLE perms (
    tag_id UUID NOT NULL,
    user_id UUID NOT NULL,

    -- can always read tasks whenever a row exists here
    can_edit BOOLEAN NOT NULL,
        -- SetTitle, EditComment for first comment
    can_triage BOOLEAN NOT NULL,
        -- SetDone, SetArchived, BlockedUntil/ScheduleFor
        -- AddTag for tags that are already on the task (setting prio/backlog)
    can_relabel_to_any BOOLEAN NOT NULL,
        -- Add/Rm-Tag for all tags (beware privilege escalation)
    can_comment BOOLEAN NOT NULL,
        -- AddComment to a date after the first comment

    PRIMARY KEY (tag_id, user_id),
    FOREIGN KEY (tag_id) REFERENCES tags (id)
        ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users (id)
        ON DELETE CASCADE
);
