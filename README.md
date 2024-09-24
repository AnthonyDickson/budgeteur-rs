# Budgeteur-rs

Budgeteur is a budgeting and personal finance web-app.

This app aims to provide two services:

- Budgeting: Recording your income and expenses, and tracking savings targets.
- Personal Finance: Keeping track of your net worth.

The application consists of a single REST server that renders and serves HTML directly.

## Quick Start

This project was developed with cargo 1.8.0, other versions have not been tested.

1.  (First time only) Run the below script to create the test database:
    ```shell
    cargo run --bin create_test_db -- --output-path test.db
    ```
2.  To start the server run the following command:

    ```shell
    SECRET=YOUR_SECRET_HERE cargo run -- --db-path test.db --cert-path path/to/cert_and_key_pem
    ```

    By default, this will serve on port 3000.

    If you want to automatically recompile and restart the server you can use the following command:

    ```shell
    cargo watch -E SECRET=YOUR_SECRET_HERE -x 'run -- --db-path test.db --cert-path path/to/cert_and_key_pem'
    ```

    `--cert-path` should contain the files `cert.pem` and `key.pem`.
    If you do not have the required SSL certificates, you can generate your own [using OpenSSL](https://stackoverflow.com/a/10176685) (make sure to add the flag `-nodes`).

3.  Test that the server is running:

    ```shell
    curl -i -X GET https://localhost:3000/coffee
    ```

    Example output:

    ```
    HTTP/2 418
    content-length: 0
    date: Thu, 22 Aug 2024 03:00:58 GMT
    ```

## Nix Development Environment

If you have Nix installed, use `nix develop` while in the root directory to create the development environment.
This creates a new shell environment with the packages declared in `flake.nix`.
Similar to a Docker image, this shell environment is isolated from your system and enables easy, replicable development environments.

## Status and Next Steps

Currently, user registration, log in and log out have been implemented on both the backend and frontend.

![Preview of the registration page.](readme/register.png)
![Preview of the log in page.](readme/log_in.png)
![Preview of the dashboard page with a log out button.](readme/dashboard.png)

The next steps will be to work on CRUD functionality for transactions.
This will allow for the fundamental budgeting functionality.
