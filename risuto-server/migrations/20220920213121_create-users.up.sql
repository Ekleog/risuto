CREATE TABLE users (
    id VARCHAR PRIMARY KEY NOT NULL,
    name VARCHAR NOT NULL UNIQUE,
    password VARCHAR NOT NULL
)