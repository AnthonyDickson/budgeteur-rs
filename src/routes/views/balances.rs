//! Displays accounts and their balances.

use askama_axum::IntoResponse;
use askama_axum::Template;
use axum::{extract::State, response::Response};

use crate::models::Balance;
use crate::routes::{
    endpoints,
    navigation::{NavbarTemplate, get_nav_bar},
};
use crate::state::BalanceState;
use crate::stores::BalanceStore;

/// Renders the balances page.
#[derive(Template)]
#[template(path = "views/balances.html")]
struct BalancesTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    balances: &'a [Balance],
    import_page_link: &'a str,
}

/// Renders the page for creating a transaction.
pub async fn get_balances_page<B>(State(state): State<BalanceState<B>>) -> Response
where
    B: BalanceStore + Send + Sync,
{
    let balances = match state.balance_store.get_all() {
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

    use axum::{extract::State, http::StatusCode, response::Response};
    use scraper::{ElementRef, Html, Selector};
    use time::{Date, macros::date};

    use crate::{
        Error,
        models::Balance,
        routes::{endpoints, views::balances::get_balances_page},
        state::BalanceState,
        stores::BalanceStore,
    };

    struct StubBalanceStore {
        balances: Vec<Balance>,
    }

    impl BalanceStore for StubBalanceStore {
        fn upsert(
            &mut self,
            _account: &str,
            _balance: f64,
            _date: &Date,
        ) -> Result<Balance, Error> {
            todo!()
        }

        fn get_all(&self) -> Result<Vec<Balance>, Error> {
            Ok(self.balances.clone())
        }
    }

    #[tokio::test]
    async fn test_get_balances_view() {
        let balances = vec![Balance {
            id: 0,
            account: "1234-5678-9101-12".to_string(),
            balance: 1234.56,
            date: date!(2025 - 05 - 31),
        }];
        let state = BalanceState {
            balance_store: StubBalanceStore {
                balances: balances.clone(),
            },
        };

        let response = get_balances_page(State(state)).await;

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

        let response = get_balances_page(State(state)).await;

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

        for (row, (table_row, want)) in zip(table_rows, balances).enumerate() {
            let got_account: String = table_row
                .select(&row_header_selector)
                .next()
                .expect(&format!(
                    "Could not find table header <th> in table row {row}."
                ))
                .text()
                .collect();
            let columns: Vec<ElementRef<'_>> = table_row.select(&row_cell_selector).collect();
            assert_eq!(
                2,
                columns.len(),
                "Want 2 table cells <td> in table row {row}, got {}",
                columns.len()
            );
            let got_balance: String = columns[0].text().collect();
            let got_date: String = columns[1].text().collect();

            assert_eq!(
                want.account, got_account,
                "want account '{}', got '{got_account}'.",
                want.account
            );
            assert_eq!(
                format!("${}", want.balance),
                got_balance,
                "want balance ${}, got {got_balance}.",
                want.balance
            );
            assert_eq!(
                want.date.to_string(),
                got_date,
                "want date {}, got {got_date}",
                want.date
            );
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
