//! Displays accounts and their balances.

use askama_axum::IntoResponse;
use askama_axum::Template;
use axum::{Extension, extract::State, response::Response};

use crate::{
    AppState,
    models::UserID,
    routes::{
        endpoints,
        navigation::{NavbarTemplate, get_nav_bar},
    },
    stores::{CategoryStore, TransactionStore, UserStore},
};

struct Balance<'a> {
    account: &'a str,
    balance: f64,
}

/// Renders the balances page.
#[derive(Template)]
#[template(path = "views/balances.html")]
struct BalancesTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    balances: &'a [Balance<'a>],
}

/// Renders the page for creating a transaction.
pub async fn get_balances_page<C, T, U>(
    State(state): State<AppState<C, T, U>>,
    Extension(user_id): Extension<UserID>,
) -> Response
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    BalancesTemplate {
        nav_bar: get_nav_bar(endpoints::BALANCES_VIEW),
        balances: &[Balance {
            account: "1234-5678-9101-12",
            balance: 1234.56,
        }],
    }
    .into_response()
}

mod tests {
    use axum::{Extension, extract::State, http::StatusCode, response::Response};

    use crate::{
        AppState,
        models::UserID,
        routes::views::balances::get_balances_page,
        stores::{CategoryStore, TransactionStore, UserStore},
    };

    #[tokio::test]
    async fn test_get_balances_view() {
        let state = AppState::new(
            "foo",
            DummyCategoryStore {},
            DummyTransactionStore {},
            DummyUserStore {},
        );
        let user_id = UserID::new(1);

        let response = get_balances_page(State(state), Extension(user_id)).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_content_type(&response, "text/html; charset=utf-8");
    }

    #[track_caller]
    fn assert_content_type(response: &Response, content_type: &str) {
        let content_type_header = response
            .headers()
            .get("content-type")
            .expect("content-type header missing");
        assert_eq!(content_type_header, content_type);
    }

    struct DummyCategoryStore;

    impl CategoryStore for DummyCategoryStore {
        fn create(
            &self,
            name: crate::models::CategoryName,
            user_id: crate::models::UserID,
        ) -> Result<crate::models::Category, crate::Error> {
            todo!()
        }

        fn get(
            &self,
            category_id: crate::models::DatabaseID,
        ) -> Result<crate::models::Category, crate::Error> {
            todo!()
        }

        fn get_by_user(
            &self,
            user_id: crate::models::UserID,
        ) -> Result<Vec<crate::models::Category>, crate::Error> {
            todo!()
        }
    }

    struct DummyTransactionStore;

    impl TransactionStore for DummyTransactionStore {
        fn create(
            &mut self,
            amount: f64,
            user_id: crate::models::UserID,
        ) -> Result<crate::models::Transaction, crate::Error> {
            todo!()
        }

        fn create_from_builder(
            &mut self,
            builder: crate::models::TransactionBuilder,
        ) -> Result<crate::models::Transaction, crate::Error> {
            todo!()
        }

        fn import(
            &mut self,
            builders: Vec<crate::models::TransactionBuilder>,
        ) -> Result<Vec<crate::models::Transaction>, crate::Error> {
            todo!()
        }

        fn get(
            &self,
            id: crate::models::DatabaseID,
        ) -> Result<crate::models::Transaction, crate::Error> {
            todo!()
        }

        fn get_by_user_id(
            &self,
            user_id: crate::models::UserID,
        ) -> Result<Vec<crate::models::Transaction>, crate::Error> {
            todo!()
        }

        fn get_query(
            &self,
            query: crate::stores::transaction::TransactionQuery,
        ) -> Result<Vec<crate::models::Transaction>, crate::Error> {
            todo!()
        }
    }

    struct DummyUserStore;

    impl UserStore for DummyUserStore {
        fn create(
            &mut self,
            email: email_address::EmailAddress,
            password_hash: crate::models::PasswordHash,
        ) -> Result<crate::models::User, crate::Error> {
            todo!()
        }

        fn get(&self, id: crate::models::UserID) -> Result<crate::models::User, crate::Error> {
            todo!()
        }

        fn get_by_email(
            &self,
            email: &email_address::EmailAddress,
        ) -> Result<crate::models::User, crate::Error> {
            todo!()
        }
    }
}
