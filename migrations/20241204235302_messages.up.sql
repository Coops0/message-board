CREATE TABLE IF NOT EXISTS messages
(
    id         UUID                                         NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    content    TEXT                                         NOT NULL,
    author     UUID REFERENCES users (id) ON DELETE CASCADE NOT NULL,
    published  BOOLEAN                                      NOT NULL DEFAULT FALSE,
    -- profanity score, 0 -> safe, 1 -> worst
    score      float4                                       NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ                                  NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_messages_published
    ON messages (published);