# backrooms-rs
Backrooms-rs is a budgeting and personal finance web-app.

This app provides two services:
* Budgeting: Recording your income and expenses, and tracking savings targets.
* Personal Finance: Keeping track of your net worth.

The application is separated into a web-based `frontend` and a REST API `backend`.

## Quickstart
### API
1.  (First time only) Run the below script to create the test database:
    ```shell
    cargo run --bin create_test_db test.db
    ```
2.  To start the API server run the following command:
    ```shell
    JWT_SECRET=YOUR_SECRET_HERE cargo run -- --db-path test.db --cert-path path/to/cert_and_key_pem
    ```
    By default this will serve on port 3000.

    `--cert-path` should contain the files `cert.pem` and `key.pem`.
    If you do not have the required SSL certificates, you can generate your own [using OpenSSL](https://stackoverflow.com/a/10176685).
3.  Test that the API is running:
    ```shell
    curl -i -X GET https://localhost:3000/api
    ```

    Example output:
    ```
    HTTP/2 418
    content-length: 0
    date: Thu, 22 Aug 2024 03:00:58 GMT
    ```

### Web Server
1.  (First time only) Add the WebAssembly target:
    ```shell
    rustup target add wasm32_unknown-unknown
    ```
2.  (First time only) Install trunk:
    ```shell
    cargo install --locked trunk
    ```
3.  Change directory into the frontend workspace:
    ```shell
    cd frontend/
    ```
4.  Start the webserver:
    ```shell
    trunk serve --open
    ```
