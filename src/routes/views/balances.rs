//! Displays accounts and their balances.

use askama_axum::IntoResponse;
use axum::{Extension, extract::State, http::StatusCode, response::Response};

use crate::{
    AppState,
    models::UserID,
    stores::{CategoryStore, TransactionStore, UserStore},
};

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
    (StatusCode::OK, "Balances Page").into_response()
}

mod tests {
    use axum::{Extension, extract::State, http::StatusCode};

    use crate::{
        AppState,
        models::UserID,
        routes::views::balances::get_balances_page,
        stores::{CategoryStore, TransactionStore, UserStore},
    };

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
    }
}
