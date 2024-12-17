use crate::user::User;
use rustrict::Type;
use sqlx::{FromRow, PgPool};
use std::cell::LazyCell;

const TYPE_SCORE_MAP: &[(Type, f32)] = &[
    (Type::PROFANE, -1.0),
    (Type::OFFENSIVE, 3.0),
    (Type::SEXUAL, 3.0),
    (Type::MEAN, 0.5),
    (Type::EVASIVE, 0.5),
    (Type::SPAM, 1.0),
    (Type::MILD, -1.0),
    (Type::MODERATE, 2.5),
    (Type::SEVERE, 6.0)
];

const MAX_TYPE_SCORE: LazyCell<f32> =
    LazyCell::new(|| TYPE_SCORE_MAP.iter().map(|(_, s)| s).filter(|&&s| s > 0.0).sum());

#[derive(FromRow)]
struct SmallMessage {
    content: String,
    published: bool,
    score: f32
}

// max score is 6
pub fn score_content(profanity_type: Type) -> f32 {
    TYPE_SCORE_MAP
        .iter()
        .fold(0.0, |acc, (t, s)| if profanity_type.is(*t) { acc + s } else { acc })
        .clamp(0.0, 6.0)
        / *MAX_TYPE_SCORE
}

pub enum CensorOutcome {
    Allow,
    Hide,
    Block
}

// todo test this
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

    if user.banned {
        return CensorOutcome::Hide;
    }

    if profanity_type.is(Type::SEVERE) {
        return CensorOutcome::Hide;
    }

    let messages = sqlx::query_as!(
            SmallMessage,
            // language=postgresql
            "SELECT content, published, score FROM messages WHERE author = $1 ORDER BY created_at DESC LIMIT 20",
            user.id
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default();

    if matches!(messages.first(), Some(m) if m.content == content) {
        return CensorOutcome::Block;
    }

    let unpublished_count = messages.iter().filter(|m| !m.published).count();
    if unpublished_count == 20 {
        return CensorOutcome::Hide;
    }

    #[allow(clippy::cast_precision_loss)]
    let average_score = messages.iter().map(|m| m.score).sum::<f32>() / messages.len() as f32;

    if average_score > 0.5 && score > 0.5 {
        return CensorOutcome::Hide;
    }

    if score >= 0.8 {
        return CensorOutcome::Hide;
    }

    CensorOutcome::Allow
}

// let Ok(existing) = ExistingMessages::fetch_for(&pool, &user).await else {
//     return;
// };

// if existing.should_block_message(&content) {
//     return;
// }

// let flagged =   existing.should_flag_message(&content)

// fn should_flag_message(content: &str) -> bool {
//     if self.flagged_count > 25 {
//         return true;
//     }
//
//     let profanity_type = Censor::from_str(content).analyze();
//
//     profanity_type.is(Type::SEVERE)
// }
