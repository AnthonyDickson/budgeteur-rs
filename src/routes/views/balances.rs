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
    import_page_link: &'a str,
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
        import_page_link: endpoints::IMPORT_VIEW,
    }
    .into_response()
}

#[cfg(test)]
mod balances_view_tests {
    use std::iter::zip;

    use axum::{Extension, extract::State, http::StatusCode, response::Response};
    use scraper::{ElementRef, Html, Selector};

    use crate::{
        Error,
        models::{Balance, DatabaseID, UserID},
        routes::{endpoints, views::balances::get_balances_page},
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
            balance_store: StubBalanceStore {
                balances: balances.clone(),
            },
        };

        let response = get_balances_page(State(state), Extension(UserID::new(1))).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_content_type(&response, "text/html; charset=utf-8");
        let html = parse_html(response).await;
        assert_valid_html(&html);
        let table = must_get_table(&html);
        assert_table_contains_balances(table, &balances);
    }

    #[tokio::test]
    async fn test_get_balances_view_no_data() {
        let balances = vec![];
        let state = BalanceState {
            balance_store: StubBalanceStore {
                balances: balances.clone(),
            },
        };

        let response = get_balances_page(State(state), Extension(UserID::new(1))).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_content_type(&response, "text/html; charset=utf-8");
        let html = parse_html(response).await;
        assert_valid_html(&html);
        let paragraph = must_get_no_data_paragraph(&html);
        assert_paragraph_contains_link(paragraph, endpoints::IMPORT_VIEW);
    }

    #[track_caller]
    fn must_get_table(html: &Html) -> ElementRef<'_> {
        let table_selector = Selector::parse("table").unwrap();
        html.select(&table_selector)
            .next()
            .expect("Could not find table in HTML")
    }

    #[track_caller]
    fn must_get_table_rows(table: ElementRef<'_>, want_row_count: usize) -> Vec<ElementRef<'_>> {
        let table_row_selector = Selector::parse("tbody tr").unwrap();
        let table_rows = table.select(&table_row_selector).collect::<Vec<_>>();

        assert_eq!(
            table_rows.len(),
            want_row_count,
            "want {want_row_count} table row, got {}",
            table_rows.len()
        );

        table_rows
    }

    #[track_caller]
    fn assert_table_contains_balances(table: ElementRef<'_>, balances: &[Balance]) {
        let table_rows = must_get_table_rows(table, balances.len());
        let row_header_selector = Selector::parse("th").unwrap();
        let row_cell_selector = Selector::parse("td").unwrap();

        for (row, (table_row, want_balance)) in zip(table_rows, balances).enumerate() {
            let got_account: String = table_row
                .select(&row_header_selector)
                .next()
                .expect(&format!(
                    "Could not find table header <th> in table row {row}."
                ))
                .text()
                .collect();
            let got_balance: String = table_row
                .select(&row_cell_selector)
                .next()
                .expect(&format!(
                    "Could not find table cell <td> in table row {row}."
                ))
                .text()
                .collect();

            assert_eq!(want_balance.account, got_account);
            assert_eq!(format!("${}", want_balance.balance), got_balance);
        }
    }

    #[track_caller]
    fn must_get_no_data_paragraph(html: &Html) -> ElementRef<'_> {
        let paragraph_selector = Selector::parse("p.no-data").unwrap();
        html.select(&paragraph_selector)
            .next()
            .expect("Could not find paragraph with class 'no-data' in HTML")
    }
    #[track_caller]
    fn assert_paragraph_contains_link(paragraph: ElementRef<'_>, want_url: &str) {
        let link_selector = Selector::parse("a").unwrap();
        let link = paragraph
            .select(&link_selector)
            .next()
            .expect("Could not find link element in paragraph.");
        let link_target = link
            .attr("href")
            .expect("Link element does define an href attribute.");

        assert_eq!(
            want_url, link_target,
            "want link with href = \"{want_url}\", but got \"{link_target}\""
        );
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
