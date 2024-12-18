use crate::user::User;
use chrono::{Duration, Utc};
use rustrict::Type;
use sqlx::{FromRow, PgPool};
use std::cell::LazyCell;

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

const SPAM_THRESHOLD: f32 = 0.4;
const HARASSMENT_THRESHOLD: f32 = 0.75;
const AUTO_HIDE_THRESHOLD: f32 = 0.55;
const SEVERE_CONTENT_THRESHOLD: f32 = 0.85;
const RATE_LIMIT_MS: i64 = 350;
const MAX_MSGS_PER_MIN: usize = 12;
const MAX_UNPUBLISHED: usize = 20;

const MAX_TYPE_SCORE: LazyCell<f32> =
    LazyCell::new(|| TYPE_SCORE_MAP.iter().map(|(_, s)| s).filter(|&&s| s > 0.0).sum());

#[derive(FromRow)]
struct SmallMessage {
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

    (raw_score / *MAX_TYPE_SCORE).clamp(0.0, 1.0)
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

    if user.banned || profanity_type.is(Type::SEVERE) || score >= SEVERE_CONTENT_THRESHOLD {
        return CensorOutcome::Hide;
    }

    let messages = sqlx::query_as!(
        SmallMessage,
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
        if now - latest.created_at < Duration::milliseconds(RATE_LIMIT_MS) && messages.len() >= 3 {
            return CensorOutcome::Block;
        }
    }

    let recent_count =
        messages.iter().take(10).filter(|m| now - m.created_at < Duration::minutes(1)).count();

    if recent_count >= MAX_MSGS_PER_MIN {
        return CensorOutcome::Hide;
    }

    let unpublished = messages.iter().filter(|m| !m.published).count();
    if unpublished >= MAX_UNPUBLISHED {
        return CensorOutcome::Hide;
    }

    #[allow(clippy::cast_precision_loss)]
    let avg_score =
        messages.iter().take(5).map(|m| m.score).sum::<f32>() / 5.0_f32.min(messages.len() as f32);

    if avg_score > HARASSMENT_THRESHOLD && score > SPAM_THRESHOLD {
        return CensorOutcome::Hide;
    }

    if score > AUTO_HIDE_THRESHOLD {
        return CensorOutcome::Hide;
    }

    CensorOutcome::Allow
}
