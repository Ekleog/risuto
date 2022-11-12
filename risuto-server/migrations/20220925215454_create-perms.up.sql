CREATE TABLE perms (
    tag_id UUID NOT NULL,
    user_id UUID NOT NULL,

    -- can always read tasks when a row exists
    can_tag BOOLEAN NOT NULL, -- add/remove tasks that user_id owns to tag
    can_comment BOOLEAN NOT NULL, -- add comments to task
    can_close BOOLEAN NOT NULL, -- close a task as done

    PRIMARY KEY (tag_id, user_id),
    FOREIGN KEY (tag_id) REFERENCES tags (id)
        ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users (id)
        ON DELETE CASCADE
)
