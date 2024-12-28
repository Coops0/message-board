use crate::user::User;
use chrono::{Duration, Utc};
use rustrict::Type;
use sqlx::{FromRow, PgPool};
use std::{cell::LazyCell, str::FromStr};

const TYPE_SCORE_MAP: &[(Type, f32)] = &[
    (Type::PROFANE, -0.1),
    (Type::OFFENSIVE, 4.0),
    (Type::SEXUAL, 1.2),
    (Type::MEAN, 2.5),
    (Type::EVASIVE, 1.8),
    (Type::SPAM, 3.0),
    (Type::MILD, -0.2),
    (Type::MODERATE, 2.0),
    (Type::SEVERE, 15.0)
];

struct Thresholds {
    spam: f32,
    harassment: f32,
    auto_hide: f32,
    severe_content: f32,
    rate_limit_ms: i64,
    max_msgs_per_min: usize,
    max_unpublished: usize
}

fn env_or<T: FromStr>(name: &str, default: T) -> T {
    dotenvy::var(name).ok().and_then(|s| s.parse().ok()).unwrap_or(default)
}

const THRESHOLDS: LazyCell<Thresholds> = LazyCell::new(|| Thresholds {
    spam: env_or("SPAM_THRESHOLD", 0.4),
    harassment: env_or("HARASSMENT_THRESHOLD", 0.75),
    auto_hide: env_or("AUTO_HIDE_THRESHOLD", 0.55),
    severe_content: env_or("SEVERE_CONTENT_THRESHOLD", 0.85),
    rate_limit_ms: env_or("RATE_LIMIT_MS", 350),
    max_msgs_per_min: env_or("MAX_MSGS_PER_MIN", 12),
    max_unpublished: env_or("MAX_UNPUBLISHED", 20)
});

const SCORE_UPPER_BOUND: LazyCell<f32> = LazyCell::new(|| {
    let mut threshes = TYPE_SCORE_MAP.iter().map(|(_, s)| *s).collect::<Vec<_>>();

    threshes.sort_by(|a, b| b.total_cmp(a));

    threshes.iter().take(2).sum()
});

#[derive(FromRow)]
struct PartialMessage {
    content: String,
    published: bool,
    score: f32,
    created_at: chrono::DateTime<Utc>
}

pub fn score_content(profanity_type: Type) -> f32 {
    let raw_score =
        TYPE_SCORE_MAP
            .iter()
            .fold(0.0, |acc, (t, s)| if profanity_type.is(*t) { acc + s } else { acc });

    (raw_score / *SCORE_UPPER_BOUND).clamp(0.0, 1.0)
}

#[derive(Debug)]
pub enum CensorOutcome {
    Allow,
    Hide,
    Block
}

pub async fn censor(
    pool: &PgPool,
    user: &User,
    content: &str,
    score: f32,
    profanity_type: Type
) -> CensorOutcome {
    if user.admin {
        return CensorOutcome::Allow;
    }

    if user.banned || profanity_type.is(Type::SEVERE) || score >= THRESHOLDS.severe_content {
        return CensorOutcome::Hide;
    }

    let messages = sqlx::query_as!(
        PartialMessage,
        // language=postgresql
        "SELECT content, published, score, created_at FROM messages
         WHERE author = $1 ORDER BY created_at DESC LIMIT 20",
        user.id
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    if matches!(messages.first(), Some(m) if m.content == content) {
        return CensorOutcome::Block;
    }

    let now = Utc::now();
    if let Some(latest) = messages.first() {
        if now - latest.created_at < Duration::milliseconds(THRESHOLDS.rate_limit_ms)
            && messages.len() >= 3
        {
            return CensorOutcome::Block;
        }
    }

    let recent_count =
        messages.iter().take(10).filter(|m| now - m.created_at < Duration::minutes(1)).count();

    if recent_count >= THRESHOLDS.max_msgs_per_min {
        return CensorOutcome::Hide;
    }

    let unpublished = messages.iter().filter(|m| !m.published).count();
    if unpublished >= THRESHOLDS.max_unpublished {
        return CensorOutcome::Hide;
    }

    #[allow(clippy::cast_precision_loss)]
    let avg_score =
        messages.iter().take(5).map(|m| m.score).sum::<f32>() / 5.0_f32.min(messages.len() as f32);

    if avg_score > THRESHOLDS.harassment && score > THRESHOLDS.spam {
        return CensorOutcome::Hide;
    }

    if score > THRESHOLDS.auto_hide {
        return CensorOutcome::Hide;
    }

    CensorOutcome::Allow
}
