CREATE TABLE IF NOT EXISTS messages
(
    id         UUID                                         NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    content    TEXT                                         NOT NULL,
    author     UUID REFERENCES users (id) ON DELETE CASCADE NOT NULL,
    flagged    BOOLEAN                                      NOT NULL DEFAULT FALSE,
    published  BOOLEAN                                      NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ                                  NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_messages_published
    ON messages (published);