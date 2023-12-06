CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE registrations (
    id    UUID    PRIMARY KEY DEFAULT public.uuid_generate_v4(),
    email TEXT    UNIQUE,
    name  TEXT,
    inserted_at   TIMESTAMPTZ NOT NULL DEFAULT now()
)
;
