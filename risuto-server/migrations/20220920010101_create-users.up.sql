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
    login TIMESTAMP NOT NULL,
    last_active TIMESTAMP NOT NULL,

    FOREIGN KEY (user_id) REFERENCES users (id)
        ON DELETE CASCADE
);

INSERT INTO users
    (id, name, password)
VALUES
    ('00000000-0000-0000-0000-000000000000', 'admin', '5e884898da28047151d0e56f8dc6292773603d0d6aabbdd62a11ef721d1542d8');
    -- sha256 of "password"
