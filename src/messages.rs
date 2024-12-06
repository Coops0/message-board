#![allow(non_snake_case)]

use leptos::prelude::*;
use leptos_use::{use_cookie_with_options, UseCookieOptions};
use serde::{Deserialize, Serialize};
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
        // language=postgresql
        "SELECT content, created_at, id FROM messages
                           WHERE (published OR fingerprint = $1)
                           ORDER BY created_at DESC LIMIT 50",
    )
    .bind(&fingerprint)
    .fetch_all(&pool)
    .await
    .map_err(Into::into)
}

#[component]
pub fn Messages() -> impl IntoView {
    // todo make this a middleware and only on the server 
    let (local_storage_id, set_local_storage_id) =
        use_cookie_with_options::<String, codee::string::Base64<Uuid>>(
            "__cf",
            UseCookieOptions::default()
                .default_value(Some(Uuid::new_v4()))
                .max_age(Some(10000 * 60 * 60 * 24 * 365))
                .build(),
        );

    let messages =
        OnceResource::<_, Postcard>::new(fetch_messages(local_storage_id.get_untracked()));

    view! {
        <Transition fallback=|| "loading...".into_view()>
            <ErrorBoundary fallback=|errs| {
                view! {
                    <div class="error">
                        <p>"u done gave me some errors here!"</p>
                        <ul>
                            {move || {
                                errs
                                    .get()
                                    .into_iter()
                                    .map(|(_, e)| view! { <li>{e.to_string()}</li> })
                                    .collect::<Vec<_>>()
                            }}
                        </ul>
                    </div>
                }
            }>
                <ul>
                    <For
                        each=move || messages.read()
                        fallback=|| "loading...".into_view()
                        key=|m| m.id
                        let:message
                    >
                        <li>{message.content}</li>
                    </For>
                </ul>
            </ErrorBoundary>
        </Transition>
    }
}
