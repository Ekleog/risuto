CREATE TABLE perms (
    tag_id VARCHAR NOT NULL,
    user_id VARCHAR NOT NULL,

    can_read BOOLEAN NOT NULL, -- read tasks
    can_tag BOOLEAN NOT NULL, -- add/remove tasks that user_id owns to tag
    can_comment BOOLEAN NOT NULL, -- add comments to task
    can_close BOOLEAN NOT NULL, -- close a task as done

    PRIMARY KEY (tag_id, user_id),
    FOREIGN KEY (tag_id) REFERENCES tags (id)
        ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users (id)
        ON DELETE CASCADE
)
