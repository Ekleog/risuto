CREATE TABLE users (
    id UUID PRIMARY KEY NOT NULL,
    name VARCHAR NOT NULL UNIQUE,
    password VARCHAR NOT NULL,

    CHECK (name ~ '^[a-zA-Z0-9]+$')
);
