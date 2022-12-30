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

    -- see risuto-api/src/event.rs for documentation of what these fields exactly enable
    can_edit BOOLEAN NOT NULL,
    can_triage BOOLEAN NOT NULL,
    can_relabel_to_any BOOLEAN NOT NULL,
    can_comment BOOLEAN NOT NULL,

    PRIMARY KEY (tag_id, user_id),
    FOREIGN KEY (tag_id) REFERENCES tags (id)
        ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users (id)
        ON DELETE CASCADE
);
