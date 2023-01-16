CREATE TABLE users (
    id UUID PRIMARY KEY NOT NULL,
    name VARCHAR NOT NULL UNIQUE,
    password VARCHAR NOT NULL,

    CHECK (name ~ '^[a-zA-Z0-9_-]+$')
);

CREATE TABLE sessions (
    id UUID PRIMARY KEY NOT NULL,
    user_id UUID NOT NULL,
    name VARCHAR NOT NULL,
    login_time TIMESTAMP NOT NULL,
    last_active TIMESTAMP NOT NULL,

    FOREIGN KEY (user_id) REFERENCES users (id)
        ON DELETE CASCADE
);
