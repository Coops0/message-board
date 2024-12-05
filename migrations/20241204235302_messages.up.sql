CREATE TABLE IF NOT EXISTS messages
(
    id                     UUID                 DEFAULT gen_random_uuid() PRIMARY KEY,
    content                TEXT        NOT NULL,
    associated_fingerprint UUID REFERENCES fingerprints (id) ON DELETE CASCADE,
    published              BOOLEAN     NOT NULL DEFAULT false,
    flagged                BOOLEAN     NOT NULL DEFAULT false,
    flag_score             SMALLINT CHECK (flag_score BETWEEN 0 AND 100),
    created_at             TIMESTAMPTZ NOT NULL DEFAULT NOW(),


    CONSTRAINT messages_flag_score_range CHECK (
        flag_score IS NULL OR (flag_score >= 0 AND flag_score <= 100)
        )
);

CREATE INDEX IF NOT EXISTS idx_messages_published
    ON messages (published);