use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub async fn get_new_category_page() -> Response {
    let mut response = (StatusCode::OK, "New Category Page").into_response();
    response.headers_mut().insert(
        "content-type",
        "text/html; charset=utf-8"
            .parse()
            .expect("valid header value"),
    );
    // TODO: Render HTML template with askama

    response
}

#[cfg(test)]
mod new_category_tests {
    use axum::http::StatusCode;

    use crate::routes::views::new_category::get_new_category_page;

    #[tokio::test]
    async fn render_page() {
        let response = get_new_category_page().await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .expect("content-type header missing"),
            "text/html; charset=utf-8"
        );
        // TODO: check that body is valid html
        // TODO: check that there is a form
        // TODO: check that the form has a hx-post attribute to the correct endpoint
        // TODO: check that the form has a text input called 'name'
        // TODO: check that the form has a submit button
    }

    #[tokio::test]
    async fn error_on_invalid_name() {
        // TODO: check that submitting an empty name results in an error being displayed
    }
}
