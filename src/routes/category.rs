//! This files defines the API routes for the category type.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Form, Json,
};
use axum_extra::extract::PrivateCookieJar;

use serde::{Deserialize, Serialize};

use crate::{
    auth::get_user_id_from_auth_cookie,
    models::{CategoryName, DatabaseID, UserID},
    stores::{CategoryStore, TransactionStore, UserStore},
    AppError, AppState,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct CategoryData {
    pub name: String,
}

/// A route handler for creating a new category.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn create_category<C, T, U>(
    State(state): State<AppState<C, T, U>>,
    Path(user_id): Path<UserID>,
    _: PrivateCookieJar,
    Form(new_category): Form<CategoryData>,
) -> impl IntoResponse
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    let name = CategoryName::new(&new_category.name)?;

    state
        .category_store()
        .create(name, user_id)
        .map(|category| (StatusCode::OK, Json(category)))
        .map_err(AppError::CategoryError)
}

/// A route handler for getting a category by its database ID.
///
/// This function will return the status code 404 if the requested resource does not exist (e.g., not created yet).
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn get_category<C, T, U>(
    State(state): State<AppState<C, T, U>>,
    jar: PrivateCookieJar,
    Path(category_id): Path<DatabaseID>,
) -> impl IntoResponse
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    state
        .category_store()
        .select(category_id)
        .map_err(AppError::CategoryError)
        .and_then(|category| {
            let user_id = get_user_id_from_auth_cookie(jar)?;

            if user_id == category.user_id() {
                Ok(category)
            } else {
                // Respond with 404 not found so that unauthorized users cannot know whether another user's resource exists.
                Err(AppError::NotFound)
            }
        })
        .map(|category| (StatusCode::OK, Json(category)))
}

#[cfg(test)]
mod category_tests {
    use axum_extra::extract::cookie::Cookie;
    use axum_test::TestServer;
    use rusqlite::Connection;

    use crate::auth::LogInData;
    use crate::build_router;
    use crate::routes::endpoints::format_endpoint;
    use crate::routes::register::RegisterForm;
    use crate::stores::sql_store::{create_app_state, SQLAppState};
    use crate::{
        auth::COOKIE_USER_ID,
        models::{Category, CategoryName, UserID},
        routes::endpoints,
    };

    use super::CategoryData;

    fn get_test_app_config() -> SQLAppState {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");

        create_app_state(db_connection, "42").unwrap()
    }

    async fn create_app_with_user() -> (TestServer, UserID, Cookie<'static>) {
        let state = get_test_app_config();
        let app = build_router(state.clone());

        let server = TestServer::new(app).expect("Could not create test server.");

        let email = "test@test.com";
        let password = "averylongandsecurepassword";

        let response = server
            .post(endpoints::USERS)
            .form(&RegisterForm {
                email: email.to_string(),
                password: password.to_string(),
                confirm_password: password.to_string(),
            })
            .await;

        response.assert_status_see_other();

        let auth_cookie = response.cookie(COOKIE_USER_ID);

        // TODO: Implement a way to get the user id from the auth cookie. For now, just guess the user id.
        (server, UserID::new(1), auth_cookie)
    }

    async fn create_app_with_user_and_category() -> (TestServer, UserID, Cookie<'static>, Category)
    {
        let (server, user_id, auth_cookie) = create_app_with_user().await;

        let category = server
            .post(&format_endpoint(
                endpoints::USER_CATEGORIES,
                user_id.as_i64(),
            ))
            .add_cookie(auth_cookie.clone())
            .content_type("application/json")
            .form(&CategoryData {
                name: "foo".to_string(),
            })
            .await
            .json::<Category>();

        (server, user_id, auth_cookie, category)
    }

    #[tokio::test]
    async fn create_category() {
        let (server, user_id, auth_cookie) = create_app_with_user().await;

        let name = CategoryName::new("Foo").unwrap();

        let response = server
            .post(&format_endpoint(
                endpoints::USER_CATEGORIES,
                user_id.as_i64(),
            ))
            .add_cookie(auth_cookie)
            .content_type("application/json")
            .form(&CategoryData {
                name: String::from("Foo"),
            })
            .await;

        response.assert_status_ok();

        let category = response.json::<Category>();

        assert_eq!(category.name(), &name);
        assert_eq!(category.user_id(), user_id);
    }

    #[tokio::test]
    async fn get_category() {
        let (server, _, auth_cookie, category) = create_app_with_user_and_category().await;

        let response = server
            .get(&format!("{}/{}", endpoints::CATEGORIES, category.id()))
            .add_cookie(auth_cookie)
            .await;

        response.assert_status_ok();

        let selected_category = response.json::<Category>();

        assert_eq!(selected_category, category);
    }

    #[tokio::test]
    async fn get_category_fails_on_wrong_user() {
        let (server, _, _, category) = create_app_with_user_and_category().await;

        let email = "test2@test.com".to_string();
        let password = "averylongandsecurepassword".to_string();

        let response = server
            .post(endpoints::USERS)
            .form(&RegisterForm {
                email: email.clone(),
                password: password.clone(),
                confirm_password: password.clone(),
            })
            .await;

        response.assert_status_see_other();

        let auth_cookie = server
            .post(endpoints::LOG_IN)
            .form(&LogInData {
                email: email.clone(),
                password: password.clone(),
            })
            .await
            .cookie(COOKIE_USER_ID);

        server
            .get(&format!("{}/{}", endpoints::CATEGORIES, category.id()))
            .add_cookie(auth_cookie)
            .await
            .assert_status_not_found();
    }
}
