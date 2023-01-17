CREATE TYPE search_order_type AS ENUM (
    'custom',
    'tag',
    'creation_date_asc',
    'creation_date_desc',
    'last_event_date_asc',
    'last_event_date_desc',
    'scheduled_for_asc',
    'scheduled_for_desc',
    'blocked_until_asc',
    'blocked_until_desc'
);

CREATE TABLE searches (
    id UUID PRIMARY KEY NOT NULL, -- doubles as order id for events
    owner_id UUID NOT NULL,
    name TEXT NOT NULL,
    filter JSON NOT NULL,
    order_type search_order_type NOT NULL,
    priority BIGINT NOT NULL,
    -- TODO: add "last-updated" field so that a long-offline machine can't mess with searches

    -- optional part
    tag_id UUID,

    FOREIGN KEY (owner_id) REFERENCES users (id),
    FOREIGN KEY (tag_id) REFERENCES tags (id), -- TODO: ON DELETE set to creation_date_asc?

    CONSTRAINT search_is_valid CHECK (
        (order_type = 'tag' AND tag_id IS NOT NULL) OR
        (order_type != 'tag' AND tag_id IS NULL)
    )
);
