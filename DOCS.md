# Developer Docs

<!--toc:start-->

- [Developer Docs](#developer-docs)
  - [Getting Started](#getting-started)
    - [Nix Flake](#nix-flake)
    - [First Time Setup](#first-time-setup)
    - [Bacon](#bacon)
      - [Running the Server](#running-the-server)
      - [Running Tests](#running-tests)
      - [Build and View Documentation](#build-and-view-documentation)
    - [Building and Running the Docker Image Locally](#building-and-running-the-docker-image-locally)
  - [TUI Client](#tui-client)
    - [Running the TUI](#running-the-tui)
    - [First-Time Setup](#first-time-setup-1)
  - [Code Style](#code-style)
    - [Error Handling](#error-handling)

<!--toc:end-->

## Getting Started

These instructions are for people who want to compile from source and/or modify
the code.

This project was developed with cargo 1.90.0, other versions have not been tested.
[bacon](https://dystroy.org/bacon/) is used for running scripts.

**Note**: you cannot test this web app locally in Safari because it does not support secure cookies on localhost.

For design and technical specifications, see [docs/](./docs).

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

## TUI Client

The TUI client (`budgeteur_tui`) is a terminal application that connects to the
Budgeteur server over the network. It uses Ed25519-signed JWTs for
passwordless authentication — no browser cookies needed.

### Running the TUI

```shell
cargo run -p budgeteur_tui -- --url http://localhost:3000
```

The server URL can also be set via `~/.config/budgeteur/config.toml`:

```toml
server_url = "http://192.168.1.100:3000"
```

CLI flags override the config file.

### First-Time Setup

1. Generate a keypair on the TUI machine:

   ```shell
   cargo run -p budgeteur_tui -- init
   ```

   This writes the private key to `~/.local/share/budgeteur/tui_private_key`
   and prints the public key.

2. Copy the printed public key to the server machine and create
   `tui_public_keys.toml`:

   ```toml
   [[keys]]
   label = "laptop"
   public_key = "<paste-the-public-key-here>"
   ```

3. Start the server with the key file:

   ```shell
   cargo run -p budgeteur_rs --bin server -- \
     --db-path test.db \
     --tui-public-keys-path tui_public_keys.toml
   ```

4. Run the TUI:

   ```shell
   cargo run -p budgeteur_tui -- --url http://localhost:3000
   ```

The TUI signs a fresh JWT on each connection cycle and sends it as a `Bearer`
token. The server validates the JWT signature and expiry against the configured
public keys. If the JWT is invalid or expired, the server returns a `401`
response.

To revoke access, remove the key entry from `tui_public_keys.toml` and restart
the server.

## Code Style

This section documents notable code style decisions and conventions.

### Error Handling

Page endpoints should rely `Error`'s `IntoResponse` implementation to render the HTML response.
Fragment endpoints should manually insert an error message into the form, or if there's no form use the `AlertTemplate`.
All errors should be logged at the source callsite, typically with `inspect_err`.
