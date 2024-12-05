#![allow(non_snake_case)]

use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{components::{Route, Router, Routes}, path};
use crate::messages::Messages;

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone() />
                <HydrationScripts options/>
                <Stylesheet id="leptos" href="/pkg/message-board.css"/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Title text="m c ps"/>

        <Router>
            <main>
                <Routes fallback=|| "page out there??".into_view()>
                    <Route path=path!("") view=Messages/>
                </Routes>
            </main>
        </Router>
    }
}