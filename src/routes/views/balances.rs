//! Displays accounts and their balances.

use askama_axum::IntoResponse;
use askama_axum::Template;
use axum::{Extension, extract::State, response::Response};

use crate::models::Balance;
use crate::state::BalanceState;
use crate::stores::BalanceStore;
use crate::{
    models::UserID,
    routes::{
        endpoints,
        navigation::{NavbarTemplate, get_nav_bar},
    },
};

/// Renders the balances page.
#[derive(Template)]
#[template(path = "views/balances.html")]
struct BalancesTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    balances: &'a [Balance],
}

/// Renders the page for creating a transaction.
pub async fn get_balances_page<B>(
    State(state): State<BalanceState<B>>,
    Extension(user_id): Extension<UserID>,
) -> Response
where
    B: BalanceStore + Send + Sync,
{
    let balances = match state.balance_store.get_by_user_id(user_id) {
        Ok(balances) => balances,
        Err(error) => return error.into_response(),
    };

    BalancesTemplate {
        nav_bar: get_nav_bar(endpoints::BALANCES_VIEW),
        balances: &balances,
    }
    .into_response()
}

#[cfg(test)]
mod balances_view_tests {
    use axum::{Extension, extract::State, http::StatusCode, response::Response};
    use scraper::Html;

    use crate::{
        Error,
        models::{Balance, DatabaseID, UserID},
        routes::views::balances::get_balances_page,
        state::BalanceState,
        stores::BalanceStore,
    };

    struct StubBalanceStore {
        balances: Vec<Balance>,
    }

    impl BalanceStore for StubBalanceStore {
        fn create(&mut self, _account: &str, _balance: f64) -> Result<Balance, Error> {
            todo!()
        }

        fn get(&self, _id: DatabaseID) -> Result<Balance, Error> {
            todo!()
        }

        fn get_by_user_id(&self, _user_id: UserID) -> Result<Vec<Balance>, Error> {
            Ok(self.balances.clone())
        }
    }

    #[tokio::test]
    async fn test_get_balances_view() {
        let balances = vec![Balance {
            account: "1234-5678-9101-12".to_string(),
            balance: 1234.56,
        }];
        let state = BalanceState {
            balance_store: StubBalanceStore { balances },
        };
        let user_id = UserID::new(1);

        let response = get_balances_page(State(state), Extension(user_id)).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_content_type(&response, "text/html; charset=utf-8");
        let html = parse_html(response).await;
        assert_valid_html(&html);
        // TODO: Check HTML for balances.
    }

    #[track_caller]
    fn assert_content_type(response: &Response, content_type: &str) {
        let content_type_header = response
            .headers()
            .get("content-type")
            .expect("content-type header missing");
        assert_eq!(content_type_header, content_type);
    }

    async fn parse_html(response: Response) -> scraper::Html {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&body).to_string();

        scraper::Html::parse_document(&text)
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
