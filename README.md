# Budgeteur

[![Build & Test](https://github.com/AnthonyDickson/budgeteur-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/AnthonyDickson/budgeteur-rs/actions/workflows/ci.yml)
[![Build & Push Docker Image](https://github.com/AnthonyDickson/budgeteur-rs/actions/workflows/cd.yaml/badge.svg)](https://github.com/AnthonyDickson/budgeteur-rs/actions/workflows/cd.yaml)

## About

Budgeteur is a budgeting and personal finance web-app.

This app aims to provide two services:

- Budgeting: Recording your income and expenses, and tracking savings targets.
- Personal Finance: Keeping track of your net worth.

This application is intended for a single user and to be self-hosted on a home server.

![Screenshot of the dashboard page of Budgeteur](./assets/dashboard_sample.jpeg)

## Table of Contents

<!--toc:start-->

- [Budgeteur](#budgeteur)
  - [About](#about)
  - [Table of Contents](#table-of-contents)
  - [Why?](#why)
  - [Getting Started](#getting-started)
    - [First-Time Usage](#first-time-usage)
    - [Resetting Your Password](#resetting-your-password)
  - [Dates and Timezones](#dates-and-timezones)
  - [Development](#development)
    - [Nix Flake](#nix-flake)
    - [First Time Setup](#first-time-setup)
    - [Bacon](#bacon)
      - [Running the Server](#running-the-server)
      - [Running Tests](#running-tests)
      - [Build and View Documentation](#build-and-view-documentation)
    - [Building and Running the Docker Image Locally](#building-and-running-the-docker-image-locally)

<!--toc:end-->

## Why?

I started budgeting with a phone app, but I quickly ran into three main issues:

1. it required me to enter my income/expenses manually,
1. it only worked on my phone,
1. and it didn't help me with tracking my net worth.

I have tried using a spreadsheet to track my net worth, however I then ran into issues where editing this spreadsheet
from multiple devices lead to old copies overwriting the copy in my cloud storage.

Budgeteur is my attempt at a single, cross-platform application for tracking my budget and net worth.
One helpful feature of Budgeteur is that you can import transactions and track your account balances from CSV files.
These CSV can be exported from the internet banking websites for New Zealand bank accounts (ASB and Kiwibank).
This reduces the amount manual data entry significantly, making it easier to maintain the habit of tracking your
budget even when life gets busy.

## Getting Started

This application is distributed as a Docker image and Docker Compose is the recommended way of running the app.

See [compose.yaml](./compose.yaml) for an example Docker compose file.

Once you have your `compose.yaml` set up, just run:

```shell
docker compose up
```

> [!CAUTION]
> The server uses HTTP which is not secure. It is highly recommended to put the
> server behind a reverse proxy such as Nginx to serve the application over
> HTTPS, especially if hosting this app on the public internet.

### First-Time Usage

Navigate to `https://<published URL>/register` and create a user account.

### Resetting Your Password

Run the following command:

```shell
docker compose -p budgeteur exec web reset_password --db-path /app/data/budgeteur.db
```

> [!TIP]
> Refer to your `compose.yaml` for the host mount path, database filename and/or image tag.

The app only allows a single user and the following instructions will reset
the password for that sole user account.

## Dates and Timezones

The app will use, in order of priority, dates and times in:

1. the timezone specified in the CLI flags or
1. the local timezone as specified by the host operating system or
1. the UTC+00:00 timezone if the host operating system's local timezone cannot be determined.

The app will assume all dates and times from the web client use the timezone as determined above.
The CLI will accept canonical timezones as specified in <https://en.wikipedia.org/w/index.php?title=List_of_tz_database_time_zones&oldid=1309592143#List>,
e.g. "Pacific/Auckland".

## Development

These instructions are for people who want to compile from source and/or modify
the code.

This project was developed with cargo 1.89.0, other versions have not been tested.
[bacon](https://dystroy.org/bacon/) is used for running scripts.

**Note**: you cannot test this web app locally in Safari because it does not support secure cookies on localhost.

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
./scripts/build_image.sh
```

This will create an image with the tag `ghcr.io/anthonydickson/budgeteur:dev`.
Run the server with:

```shell
docker run --rm -p 8080:8080 -e SECRET=<YOUR-SECRET> -it ghcr.io/anthonydickson/budgeteur:dev
```

> [!NOTE]
> Add `-v $(pwd):/app/data` to the above command (before `-it`) to persist
> the app database after the container has stopped.
