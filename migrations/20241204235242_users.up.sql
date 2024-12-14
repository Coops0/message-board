CREATE TABLE IF NOT EXISTS users
(
    id                UUID PRIMARY KEY NOT NULL,
    code              TEXT UNIQUE      NOT NULL,
    admin             BOOLEAN          NOT NULL                                       DEFAULT FALSE,

    location_referral TEXT             REFERENCES locations (code) ON DELETE SET NULL DEFAULT NULL,
    user_referral     UUID             REFERENCES users (id) ON DELETE SET NULL       DEFAULT NULL,

    ip                TEXT             NOT NULL,
    user_agent        TEXT                                                            DEFAULT NULL,

    banned            BOOLEAN          NOT NULL                                       DEFAULT FALSE,
    created_at        TIMESTAMPTZ      NOT NULL                                       DEFAULT now()
);