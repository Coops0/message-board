CREATE TABLE IF NOT EXISTS fingerprints
(
    id               UUID PRIMARY KEY         DEFAULT gen_random_uuid(),
    ip               CIDR NOT NULL,
    user_agent       TEXT NOT NULL,
    local_storage_id UUID NOT NULL,
    last_seen        TIMESTAMP WITH TIME ZONE DEFAULT now(),
    created_at       TIMESTAMP WITH TIME ZONE DEFAULT now()
);