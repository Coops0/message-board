CREATE TABLE IF NOT EXISTS users
(
    id                UUID PRIMARY KEY,
    code              TEXT UNIQUE NOT NULL,
    admin             BOOLEAN     NOT NULL             DEFAULT FALSE,

    location_referral TEXT REFERENCES locations (code) DEFAULT NULL,
    user_referral     UUID REFERENCES users (id)       DEFAULT NULL,

    ip                TEXT        NOT NULL,
    user_agent        TEXT                             DEFAULT NULL,

    banned            BOOLEAN     NOT NULL             DEFAULT FALSE,
    created_at        TIMESTAMP WITH TIME ZONE         DEFAULT now(),

    CONSTRAINT exclusive_referral CHECK (
        NOT (location_referral IS NOT NULL AND user_referral IS NOT NULL)
        )
);