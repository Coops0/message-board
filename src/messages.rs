#![allow(non_snake_case)]

use leptos::prelude::*;
use leptos::server_fn::serde::{Deserialize, Serialize};
use server_fn::codec::Postcard;
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct Message {
    pub id: Uuid,
    pub content: String,
}

#[server(input = Postcard, output = Postcard)]
pub async fn fetch_messages(fingerprint: String) -> Result<Vec<Message>, ServerFnError> {
    use crate::AppState;

    let AppState { pool, .. } = expect_context::<AppState>();

    sqlx::query_as::<_, Message>(
        // language=PgSQL
        "SELECT content, created_at, id FROM messages
                           WHERE (published = true OR fingerprint = $1)
                           ORDER BY created_at DESC LIMIT 50",
    )
    .bind(&fingerprint)
    .fetch_all(&pool)
    .await
    .map_err(Into::into)
}

#[component]
pub fn Messages() -> impl IntoView {
    let messages = OnceResource::<_, Postcard>::new(fetch_messages("1".to_string()));

    view! {
        <Transition fallback=|| "loading...".into_view()>
         <ErrorBoundary
                fallback=|errs| view! {
                    <div class="error">
                        <p>"u done gave me some errors here!"</p>
                        <ul>
                            {move || errs.get()
                                .into_iter()
                                .map(|(_, e)| view! { <li>{e.to_string()}</li>})
                                .collect::<Vec<_>>()
                            }
                        </ul>
                    </div>
                }
            >
            <ul>
              <For each=move || messages.read() fallback=|| "loading...".into_view() key=|m| m.id let:message>
                <li>{message.content}</li>
              </For>
        </ul>
        </ErrorBoundary>
        </Transition>
    }
}
