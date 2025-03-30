use askama::Template;
use axum::{
    Extension,
    extract::State,
    response::{IntoResponse, Response},
};
use time::Date;

use crate::{
    AppState,
    models::{Category, UserID},
    routes::{
        endpoints,
        navigation::{NavbarTemplate, get_nav_bar},
    },
    stores::{CategoryStore, TransactionStore, UserStore},
};

/// Renders the dashboard page.
#[derive(Template)]
#[template(path = "views/new_transaction.html")]
struct NewTransactionTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    create_transaction_route: &'a str,
    new_category_route: &'a str,
    categories: Vec<Category>,
    max_date: Date,
}

/// Renders the page for creating a transaction.
pub async fn get_new_transaction_page<C, T, U>(
    State(state): State<AppState<C, T, U>>,
    Extension(user_id): Extension<UserID>,
) -> Response
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    let categories = state.category_store.get_by_user(user_id).unwrap();

    let nav_bar = get_nav_bar(endpoints::NEW_TRANSACTION_VIEW);

    NewTransactionTemplate {
        nav_bar,
        create_transaction_route: endpoints::TRANSACTIONS_API,
        new_category_route: endpoints::NEW_CATEGORY_VIEW,
        categories,
        max_date: time::OffsetDateTime::now_utc().date(),
    }
    .into_response()
}

#[cfg(test)]
mod new_transaction_route_tests {
    use std::collections::HashMap;

    use axum::{
        Extension,
        body::Body,
        extract::State,
        http::{StatusCode, response::Response},
    };
    use scraper::{ElementRef, Html};
    use time::OffsetDateTime;

    use crate::{
        AppState, Error,
        models::{
            Category, CategoryName, DatabaseID, PasswordHash, Transaction, TransactionBuilder,
            User, UserID,
        },
        routes::endpoints,
        stores::{CategoryStore, TransactionStore, UserStore, transaction::TransactionQuery},
    };

    use super::get_new_transaction_page;

    #[derive(Clone)]
    struct DummyUserStore {}

    impl UserStore for DummyUserStore {
        fn create(
            &mut self,
            _email: email_address::EmailAddress,
            _password_hash: PasswordHash,
        ) -> Result<User, Error> {
            todo!()
        }

        fn get(&self, _id: UserID) -> Result<User, Error> {
            todo!()
        }

        fn get_by_email(&self, _email: &email_address::EmailAddress) -> Result<User, Error> {
            todo!()
        }
    }

    #[derive(Clone)]
    struct StubCategoryStore {
        categories: Vec<Category>,
    }

    impl CategoryStore for StubCategoryStore {
        fn create(&self, _name: CategoryName, _user_id: UserID) -> Result<Category, Error> {
            todo!()
        }

        fn get(&self, _category_id: DatabaseID) -> Result<Category, Error> {
            todo!()
        }

        fn get_by_user(&self, user_id: UserID) -> Result<Vec<Category>, Error> {
            let categories = self
                .categories
                .iter()
                .map(|category| {
                    let mut new_category = category.clone();
                    new_category.user_id = user_id;
                    new_category
                })
                .collect();
            Ok(categories)
        }
    }

    struct DummyTransactionStore {}

    impl TransactionStore for DummyTransactionStore {
        fn create(&mut self, _amount: f64, _user_id: UserID) -> Result<Transaction, Error> {
            todo!()
        }

        fn create_from_builder(
            &mut self,
            _builder: TransactionBuilder,
        ) -> Result<Transaction, Error> {
            todo!()
        }

        fn get(&self, _id: DatabaseID) -> Result<Transaction, Error> {
            todo!()
        }

        fn get_by_user_id(&self, _user_id: UserID) -> Result<Vec<Transaction>, Error> {
            todo!()
        }

        fn get_query(&self, _filter: TransactionQuery) -> Result<Vec<Transaction>, Error> {
            todo!()
        }
    }

    #[tokio::test]
    async fn returns_form() {
        let user_id = UserID::new(42);
        let mut categories = vec![
            Category {
                id: 1,
                name: CategoryName::new_unchecked("foo"),
                user_id,
            },
            Category {
                id: 2,
                name: CategoryName::new_unchecked("bar"),
                user_id,
            },
        ];

        let category_store = StubCategoryStore {
            categories: categories.clone(),
        };
        // This category should be auto-generated by the view.
        categories.push(Category {
            id: 0,
            name: CategoryName::new_unchecked("None"),
            user_id,
        });

        let app_state = AppState::new(
            "foobar",
            category_store,
            DummyTransactionStore {},
            DummyUserStore {},
        );

        let response = get_new_transaction_page(State(app_state), Extension(user_id)).await;

        assert_status_ok(&response);
        assert_html_content_type(&response);

        let document = parse_html(response).await;
        assert_correct_form(&document, categories);
    }

    #[track_caller]
    fn assert_status_ok(response: &Response<Body>) {
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[track_caller]
    fn assert_html_content_type(response: &Response<Body>) {
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .unwrap()
                .to_str()
                .unwrap(),
            "text/html; charset=utf-8"
        );
    }

    #[track_caller]
    fn assert_correct_form(document: &Html, categories: Vec<Category>) {
        let form_selector = scraper::Selector::parse("form").unwrap();
        let forms = document.select(&form_selector).collect::<Vec<_>>();
        assert_eq!(forms.len(), 1, "want 1 form, got {}", forms.len());

        let form = forms.first().unwrap();
        let hx_post = form.value().attr("hx-post");
        assert_eq!(
            hx_post,
            Some(endpoints::TRANSACTIONS_API),
            "want form with attribute hx-post=\"{}\", got {:?}",
            endpoints::TRANSACTIONS_API,
            hx_post
        );

        assert_correct_inputs(form);
        assert_correct_select_and_options(form, categories);
        assert_has_submit_button(form);
    }

    #[track_caller]
    fn assert_correct_inputs(form: &ElementRef) {
        let expected_input_types = vec![
            ("amount", "number"),
            ("date", "date"),
            ("description", "text"),
        ];

        for (name, element_type) in expected_input_types {
            let selector_string = format!("input[type={element_type}]");
            let input_selector = scraper::Selector::parse(&selector_string).unwrap();
            let inputs = form.select(&input_selector).collect::<Vec<_>>();
            assert_eq!(
                inputs.len(),
                1,
                "want 1 {element_type} input, got {}",
                inputs.len()
            );

            let input = inputs.first().unwrap();

            let input_name = input.value().attr("name");
            assert_eq!(
                input_name,
                Some(name),
                "want {element_type} with name=\"{name}\", got {input_name:?}"
            );

            match input_name {
                Some("amount") => {
                    assert_required(input);
                    assert_amount_min_and_step(input);
                }
                Some("date") => {
                    assert_required(input);
                    assert_max_date(input);
                    assert_value(input, &OffsetDateTime::now_utc().date().to_string());
                }
                _ => {}
            }
        }
    }

    #[track_caller]
    fn assert_value(input: &ElementRef, expected_value: &str) {
        let value = input.value().attr("value");
        assert_eq!(
            value,
            Some(expected_value),
            "want input with value=\"{expected_value}\", got {value:?}"
        );
    }

    #[track_caller]
    fn assert_required(input: &ElementRef) {
        let required = input.value().attr("required");
        let input_name = input.value().attr("name").unwrap();
        assert!(
            required.is_some(),
            "want {input_name} input to be required, got {required:?}"
        );
    }

    #[track_caller]
    fn assert_max_date(input: &ElementRef) {
        let today = OffsetDateTime::now_utc().date();
        let max_date = input.value().attr("max");

        assert_eq!(
            Some(today.to_string().as_str()),
            max_date,
            "the date for a new transaction should be limited to the current date {today}, but got {max_date:?}"
        );
    }

    #[track_caller]
    fn assert_amount_min_and_step(input: &ElementRef) {
        let min_value = input
            .value()
            .attr("min")
            .expect("amount input should have the attribute 'min'");
        let min_value: i64 = min_value
            .parse()
            .expect("the attribute 'min' for the amount input should be an integer");
        assert_eq!(
            0, min_value,
            "the amount for a new transaction should be limited to a minimum of 0, but got {min_value}"
        );

        let step = input
            .value()
            .attr("step")
            .expect("amount input should have the attribute 'step'");
        let step: f64 = step
            .parse()
            .expect("the attribute 'step' for the amount input should be a float");
        assert_eq!(
            0.01, step,
            "the amount for a new transaction should increment in steps of 0.01, but got {step}"
        );
    }

    #[track_caller]
    fn assert_correct_select_and_options(form: &ElementRef, categories: Vec<Category>) {
        let select_selector = scraper::Selector::parse("select").unwrap();
        let selects = form.select(&select_selector).collect::<Vec<_>>();
        assert_eq!(selects.len(), 1, "want 1 select tag, got {}", selects.len());
        let select_tag = selects.first().unwrap();
        let select_name = select_tag.value().attr("name");
        assert_eq!(
            select_name,
            Some("category_id"),
            "want select with name=\"category_id\", got {select_name:?}"
        );

        let select_option_selector = scraper::Selector::parse("option").unwrap();
        let options = select_tag
            .select(&select_option_selector)
            .collect::<Vec<_>>();

        assert_eq!(
            categories.len(),
            options.len(),
            "want {} options, got {}",
            categories.len(),
            options.len()
        );
        let mut category_names = HashMap::new();
        for category in categories {
            category_names.insert(category.id, category.name.clone());
        }

        for option in options {
            let option_value = option.value().attr("value");
            let option_text = option.text().collect::<String>();
            let category_id = option_value
                .unwrap()
                .parse::<i64>()
                .expect("got option with non-integer value");
            let category_name = category_names
                .get(&category_id)
                .expect("got option with unknown category id");

            assert_eq!(
                option_text,
                category_name.as_ref(),
                "want option with value=\"{category_id}\" to have text \"{category_name}\", got {option_text:?}"
            );
        }
    }

    #[track_caller]
    fn assert_has_submit_button(form: &ElementRef) {
        let button_selector = scraper::Selector::parse("button").unwrap();
        let buttons = form.select(&button_selector).collect::<Vec<_>>();
        assert_eq!(buttons.len(), 1, "want 1 button, got {}", buttons.len());
        let button_type = buttons.first().unwrap().value().attr("type");
        assert_eq!(
            button_type,
            Some("submit"),
            "want button with type=\"submit\", got {button_type:?}"
        );
    }

    async fn parse_html(response: Response<Body>) -> scraper::Html {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&body).to_string();

        scraper::Html::parse_document(&text)
    }
}
