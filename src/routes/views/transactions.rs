use askama_axum::Template;
use axum::{
    Extension,
    extract::State,
    http::Uri,
    response::{IntoResponse, Response},
};

use crate::{
    AppState,
    models::UserID,
    routes::{
        endpoints,
        navigation::{NavbarTemplate, get_nav_bar},
        templates::TransactionRow,
    },
    stores::{
        CategoryStore, TransactionStore, UserStore,
        transaction::{SortOrder, TransactionQuery},
    },
};

/// Renders the dashboard page.
#[derive(Template)]
#[template(path = "views/transactions.html")]
struct TransactionsTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    /// The user's transactions for this week, as Askama templates.
    transactions: Vec<TransactionRow>,
    /// The route for creating a new transaction for the current user.
    create_transaction_route: Uri,
    /// The route for importing transactions from CSV files.
    import_transaction_route: Uri,
}

// TODO: implement pagination
/// Render an overview of the user's transactions.
pub async fn get_transactions_page<C, T, U>(
    State(state): State<AppState<C, T, U>>,
    Extension(user_id): Extension<UserID>,
) -> Response
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    let nav_bar = get_nav_bar(endpoints::TRANSACTIONS_VIEW);

    let transactions = state.transaction_store.get_query(TransactionQuery {
        user_id: Some(user_id),
        limit: Some(20),
        sort_date: Some(SortOrder::Descending),
        ..Default::default()
    });
    let transactions = match transactions {
        Ok(transactions) => transactions,
        Err(error) => return error.into_response(),
    };

    let transactions = transactions
        .into_iter()
        .map(|transaction| TransactionRow { transaction })
        .collect();

    TransactionsTemplate {
        nav_bar,
        transactions,
        create_transaction_route: Uri::from_static(endpoints::NEW_TRANSACTION_VIEW),
        import_transaction_route: Uri::from_static(endpoints::IMPORT_VIEW),
    }
    .into_response()
}

#[cfg(test)]
mod transactions_route_tests {
    use axum::{
        Router, middleware,
        routing::{get, post},
    };
    use axum_test::TestServer;
    use rusqlite::Connection;
    use scraper::Html;

    use crate::{
        auth::{log_in::LogInData, middleware::auth_guard},
        models::{PasswordHash, Transaction, User, ValidatedPassword},
        routes::{endpoints, log_in::post_log_in},
        stores::{
            TransactionStore, UserStore,
            sql_store::{SQLAppState, create_app_state},
        },
    };

    use super::get_transactions_page;

    fn get_test_state_server_and_user() -> (SQLAppState, TestServer, User) {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");

        let mut state = create_app_state(db_connection, "42").unwrap();

        let user = state
            .user_store
            .create(
                "test@test.com".parse().unwrap(),
                PasswordHash::new(ValidatedPassword::new_unchecked("test"), 4).unwrap(),
            )
            .unwrap();

        let app = Router::new()
            .route(endpoints::TRANSACTIONS_VIEW, get(get_transactions_page))
            .layer(middleware::from_fn_with_state(state.clone(), auth_guard))
            .route(endpoints::LOG_IN_API, post(post_log_in))
            .with_state(state.clone());

        let server = TestServer::new(app).expect("Could not create test server.");

        (state, server, user)
    }

    #[tokio::test]
    async fn transactions_page_displays_correct_info() {
        let (mut state, server, user) = get_test_state_server_and_user();

        let transactions = vec![
            state
                .transaction_store
                .create_from_builder(Transaction::build(1.0, user.id()).description("foo"))
                .unwrap(),
            state
                .transaction_store
                .create_from_builder(Transaction::build(2.0, user.id()).description("bar"))
                .unwrap(),
        ];

        let jar = server
            .post(endpoints::LOG_IN_API)
            .form(&LogInData {
                email: "test@test.com".to_string(),
                password: "test".to_string(),
                remember_me: None,
            })
            .await
            .cookies();

        let transactions_page = server
            .get(endpoints::TRANSACTIONS_VIEW)
            .add_cookies(jar)
            .await;

        transactions_page.assert_status_ok();

        let transactions_page_text = transactions_page.text();
        let html = Html::parse_document(&transactions_page_text);
        assert_valid_html(&html);

        for transaction in transactions {
            assert!(transactions_page_text.contains(&transaction.date().to_string()));
            assert!(transactions_page_text.contains(transaction.description()));
        }
    }

    #[track_caller]
    fn assert_valid_html(html: &Html) {
        assert!(
            html.errors.is_empty(),
            "Got HTML parsing errors: {:?}",
            html.errors
        );
    }
}
