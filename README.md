# Budgeteur-rs

Budgeteur is a budgeting and personal finance web-app.

This app aims to provide two services:

- Budgeting: Recording your income and expenses, and tracking savings targets.
- Personal Finance: Keeping track of your net worth.

The application consists of a single server that renders and serves HTML directly.

<!--toc:start-->
- [Budgeteur-rs](#budgeteur-rs)
  - [Installation and Usage](#installation-and-usage)
  - [Set Up Development Environment](#set-up-development-environment)
    - [Nix Flake](#nix-flake)
    - [First Time Setup](#first-time-setup)
    - [Bacon](#bacon)
      - [Running the Server](#running-the-server)
      - [Running Tests](#running-tests)
      - [Build and View Documentation](#build-and-view-documentation)
    - [Building and Running the Docker Image Locally](#building-and-running-the-docker-image-locally)
  - [API Design](#api-design)
    - [HTTP Status Codes](#http-status-codes)
<!--toc:end-->

## Installation and Usage

This application is distributed as a Docker image and Docker Compose is the
recommended way of running the app.

See [compose.yaml](./compose.yaml) for an example Docker compose file.
It is set up to run a local image built with [build_image.sh](./build_image.sh),
but should be modified to use an image from the GitHub Container Registry.

Once you have your `compose.yaml` set up, just run:

```shell
docker compose up
```

> [!CAUTION]
> The server uses HTTP which is not secure. It is recommended to put the server
> behind a reverse proxy such as Nginx to serve the application over HTTPS,
> especially if hosting this app on the public internet.

## Set Up Development Environment

These instructions are for people who want to compile from source.

This project was developed with cargo 1.8.5, other versions have not been tested.
[bacon](https://dystroy.org/bacon/) is used for running scripts.

**Note**: you cannot test this web app in Safari because it does not support
secure cookies on localhost.

### Nix Flake

If you have Nix installed, use `nix develop` while in the root directory to
create the development environment.
This creates a new shell environment with the correct version of Rust, any
additional tools required for development such as `tailwindcss` and `bacon`,
and dummy environment variables, e.g. `SECRET`, for local testing.

### First Time Setup

(First time only) Run the below script to create the test database:

```shell
cargo run --bin create_test_db -- --output-path test.db
```

### Bacon

Run `bacon` in your terminal.

`bacon` is used watch for file changes and run commands.
If you cannot install `bacon`, you can run the commands manually.
See [bacon.toml](./bacon.toml) for the list of commands.

#### Running the Server

`bacon` should automatically start the server. If not, press `r` in `bacon`.

By default, this will serve on port 3000.
`bacon` will watch for changes and automatically recompile and restart the server.

Test that the server is running in another terminal:

```shell
curl -i -X GET http://localhost:3000/api/coffee
```

Example output:

```text
HTTP/2 418
content-length: 0
date: Thu, 22 Aug 2024 03:00:58 GMT
```

#### Running Tests

Run tests in `bacon` by pressing `t`.
This will watch for changes and run all the tests in the project.

#### Build and View Documentation

Build the documentation in `bacon` by pressing `d`.
This will build the documentation and open it in your default browser.

### Building and Running the Docker Image Locally

Run:

```shell
./build_image.sh
```

This will create an image with the tag `anthonydickson/budgeteur:dev`.
Run the server with:

```shell
docker run --rm -p 3000:3000 -e SECRET=<YOUR-SECRET> -it anthonydickson/budgeteur:dev
```

> [!NOTE]
> Add `-v test.db:/app/app.db` to the above command (before `-it`) to persist
> the app database after the container has stopped.

## API Design

### HTTP Status Codes

HTTP status codes are generally used in line with the standards that define
them.

2xx status codes indicate that the server understood and processed the
request without errors, and the client does not need to perform any special
handling of the response. Note that this means that things like invalid log-in
credentials or invalid emails in registrations forms will return with a HTTP
200 status code because these response will contain the error messages that
should be displayed directly to the user and there is no action the client can
or should take on the user's behalf to rectify these issues.

3xx status codes are used for full page redirects. In cases where the response
to a HTMX request requires a redirect, the corresponding HTMX redirect header
is used instead.

4xx status codes are used when the request could not be fulfilled due to
issues with the request. Common causes are requests for non-existent resources
or malformed forms (e.g., missing form fields). 4xx codes are not used to
indicate expected application errors (invalid log-in credentials).

5xx status codes are used when the request could not be fulfilled due to an
unexpected and unhandled error on the server.
