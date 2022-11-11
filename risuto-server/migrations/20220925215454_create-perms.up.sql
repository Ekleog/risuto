CREATE TABLE perms (
    tag_id UUID NOT NULL,
    user_id UUID NOT NULL,

    can_read BOOLEAN NOT NULL, -- read tasks
    can_tag BOOLEAN NOT NULL, -- add/remove tasks that user_id owns to tag
    can_comment BOOLEAN NOT NULL, -- add comments to task
    can_close BOOLEAN NOT NULL, -- close a task as done

    CONSTRAINT at_least_one_perm CHECK (
        can_read != false OR
        can_tag != false OR
        can_comment != false OR
        can_close != false
    ),

    PRIMARY KEY (tag_id, user_id),
    FOREIGN KEY (tag_id) REFERENCES tags (id)
        ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users (id)
        ON DELETE CASCADE
)
