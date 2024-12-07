CREATE TABLE IF NOT EXISTS users
(
    id                UUID PRIMARY KEY         DEFAULT gen_random_uuid(),
    code              TEXT UNIQUE NOT NULL,

    location_referral TEXT REFERENCES locations (code),
    user_referral     UUID REFERENCES users (id),

    last_seen         TIMESTAMP WITH TIME ZONE DEFAULT now(),
    ip                TEXT        NOT NULL,
    user_agent        TEXT,

    banned            BOOLEAN     NOT NULL     DEFAULT false,
    created_at        TIMESTAMP WITH TIME ZONE DEFAULT now(),

    CONSTRAINT exclusive_referral CHECK (
        NOT (location_referral IS NOT NULL AND user_referral IS NOT NULL)
        )
);