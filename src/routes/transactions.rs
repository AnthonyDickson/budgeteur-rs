use askama_axum::Template;
use axum::{
    extract::State,
    http::Uri,
    response::{IntoResponse, Response},
    Extension,
};
use time::{Date, OffsetDateTime};

use crate::{
    models::UserID, routes::get_internal_server_error_redirect, stores::TransactionStore, AppError,
    AppState,
};

use super::{
    endpoints::{self, format_endpoint},
    navigation::{get_nav_bar, NavbarTemplate},
    templates::TransactionRow,
};

/// Renders the dashboard page.
#[derive(Template)]
#[template(path = "views/transactions.html")]
struct TransactionsTemplate<'a> {
    navbar: NavbarTemplate<'a>,
    /// The user's transactions for this week, as Askama templates.
    transactions: Vec<TransactionRow>,
    /// Today's date, i.e. the date the template was rendered.
    today: Date,
    /// The route for creating a new transaction for the current user.
    create_transaction_route: Uri,
}

pub async fn get_transactions_page(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserID>,
) -> Response {
    let navbar = get_nav_bar(endpoints::TRANSACTIONS);

    // TODO: Create function for getting transactions within a time span (time::Duration).
    let transactions = state.transaction_store().get_by_user_id(user_id);
    let transactions = match transactions {
        Ok(transactions) => transactions,
        Err(error) => return AppError::TransactionError(error).into_response(),
    };

    let today = OffsetDateTime::now_utc().date();

    let create_transaction_route =
        format_endpoint(endpoints::USER_TRANSACTIONS, user_id.as_i64()).parse();

    let create_transaction_route = match create_transaction_route {
        Ok(uri) => uri,
        Err(error) => {
            tracing::error!(
                "An error ocurred while creating route URI using the endpoint {}: {error}",
                endpoints::USER_TRANSACTIONS
            );
            return get_internal_server_error_redirect();
        }
    };

    let transactions = transactions
        .into_iter()
        .map(|transaction| TransactionRow { transaction })
        .collect();

    TransactionsTemplate {
        navbar,
        transactions,
        today,
        create_transaction_route,
    }
    .into_response()
}

#[cfg(test)]
mod transactions_route_tests {
    use axum::{
        middleware,
        routing::{get, post},
        Router,
    };
    use axum_test::TestServer;
    use rusqlite::Connection;

    use crate::{
        auth::LogInData,
        models::{Transaction, User},
        routes::log_in::post_log_in,
        stores::{TransactionStore, UserStore},
    };
    use crate::{
        auth::{auth_guard, COOKIE_USER_ID},
        db::initialize,
        models::{PasswordHash, ValidatedPassword},
        routes::endpoints,
        AppState,
    };

    use super::get_transactions_page;

    fn get_test_state_server_and_user() -> (AppState, TestServer, User) {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");
        initialize(&db_connection).expect("Could not initialize database.");

        let state = AppState::new(db_connection, "42");

        let user = state
            .user_store()
            .create(
                "test@test.com".parse().unwrap(),
                PasswordHash::new(ValidatedPassword::new_unchecked("test".to_string())).unwrap(),
            )
            .unwrap();

        let app = Router::new()
            .route(endpoints::TRANSACTIONS, get(get_transactions_page))
            .layer(middleware::from_fn_with_state(state.clone(), auth_guard))
            .route(endpoints::LOG_IN, post(post_log_in))
            .with_state(state.clone());

        let server = TestServer::new(app).expect("Could not create test server.");

        (state, server, user)
    }

    #[tokio::test]
    async fn transactions_page_displays_correct_info() {
        let (state, server, user) = get_test_state_server_and_user();

        let mut transactions = Vec::new();

        transactions.push(
            state
                .transaction_store()
                .create_from_builder(
                    Transaction::build(1.0, user.id()).description("foo".to_string()),
                )
                .unwrap(),
        );

        transactions.push(
            state
                .transaction_store()
                .create_from_builder(
                    Transaction::build(2.0, user.id()).description("bar".to_string()),
                )
                .unwrap(),
        );

        let auth_cookie = server
            .post(endpoints::LOG_IN)
            .form(&LogInData {
                email: "test@test.com".to_string(),
                password: "test".to_string(),
            })
            .await
            .cookie(COOKIE_USER_ID);

        let transactions_page = server
            .get(endpoints::TRANSACTIONS)
            .add_cookie(auth_cookie)
            .await;

        transactions_page.assert_status_ok();

        let transactions_page = transactions_page.text();

        for transaction in transactions {
            assert!(transactions_page.contains(&transaction.date().to_string()));
            assert!(transactions_page.contains(transaction.description()));
        }
    }
}
