CREATE TABLE IF NOT EXISTS fingerprints
(
    id         UUID PRIMARY KEY         DEFAULT gen_random_uuid(),
    ip         TEXT    NOT NULL,
    user_agent TEXT    NOT NULL,
    banned     BOOLEAN NOT NULL         DEFAULT false,
    last_seen  TIMESTAMP WITH TIME ZONE DEFAULT now(),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now()
);