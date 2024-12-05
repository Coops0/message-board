#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::Router;
    use leptos::{
        logging::{log, warn},
        prelude::*,
    };
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use message_board::app::*;
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
    use std::env;
    use message_board::AppState;

    let _ = dotenvy::dotenv();

    let conf = get_configuration(None).unwrap();
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;

    let routes = generate_route_list(App);

    let pool = PgPoolOptions::new()
        .connect_lazy_with(env::var("DATABASE_URL")?.parse::<PgConnectOptions>()?);

    if let Err(why) = sqlx::migrate!().run(&pool).await {
        warn!("migrations failed: {:?}", why);
    } else {
        log!("migrations ran successfully / db connection valid");
    }

    let app_state = AppState {
        pool,
        leptos_options,
    };

    let app = Router::new()
        .leptos_routes_with_context(
            &app_state,
            routes,
            {
                let app_state = app_state.clone();
                move || shell(app_state)
            },
            App,
        )
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(app_state);

    log!("listening on http://{}", &addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // no client-side main function
    // unless we want this to work with e.g., Trunk for pure client-side testing
    // see lib.rs for hydration function instead
}
