CREATE TABLE IF NOT EXISTS messages
(
    id                     UUID                 DEFAULT gen_random_uuid() PRIMARY KEY,
    content                TEXT        NOT NULL,
    associated_fingerprint UUID REFERENCES fingerprints (id) ON DELETE CASCADE,
    flagged                BOOLEAN     NOT NULL DEFAULT false,
    published              BOOLEAN     NOT NULL DEFAULT false,
    created_at             TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_messages_published
    ON messages (published);