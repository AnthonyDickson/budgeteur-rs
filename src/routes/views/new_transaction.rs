use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

/// Renders the page for creating a transaction.
pub async fn get_new_transaction_page() -> Response {
    // TODO: render the new transaction page template
    (StatusCode::OK, "New transaction page").into_response()
}

#[cfg(test)]
mod new_transaction_route_tests {
    use axum::http::StatusCode;

    use super::get_new_transaction_page;

    #[tokio::test]
    async fn returns_200() {
        let result = get_new_transaction_page().await;
        assert_eq!(result.status(), StatusCode::OK);
    }
}
