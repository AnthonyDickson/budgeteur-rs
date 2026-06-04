FROM rust:1.96.0-alpine3.23 AS build

RUN apk update
RUN apk add --no-cache musl-dev 

WORKDIR /build

COPY Cargo.toml /build/Cargo.toml
COPY server/Cargo.toml /build/server/Cargo.toml
COPY tui/Cargo.toml /build/tui/Cargo.toml
COPY shared/Cargo.toml /build/shared/Cargo.toml
COPY Cargo.lock /build/Cargo.lock
COPY server/src/ /build/server/src/
COPY shared/src/ /build/shared/src/
# Make skeleton TUI project to avoid copying in the whole TUI source
RUN mkdir -p /build/tui/src && touch /build/tui/src/main.rs

RUN cargo build --verbose --release -p budgeteur_rs --bin server --bin reset_password

#==============================================================================#

FROM alpine:3.23 AS tailwind

RUN apk update
RUN apk add --no-cache curl libgcc libstdc++

WORKDIR /build
COPY server/src /build/src
RUN curl -sL https://github.com/tailwindlabs/tailwindcss/releases/download/v4.1.18/tailwindcss-linux-x64-musl -o tailwindcss && \
  chmod +x tailwindcss && \
  ./tailwindcss --input src/input.css --output static/main.css --minify

#==============================================================================#

FROM alpine:3.23 AS deploy

WORKDIR /app

COPY static/ ./static
COPY --from=tailwind /build/static/main.css /app/static/main.css
COPY --from=build /build/target/release/server /usr/local/bin/server
COPY --from=build /build/target/release/reset_password /usr/local/bin/reset_password

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD wget --no-verbose --tries=1 --spider http://localhost:8080/api/health || exit 1

CMD [ "server", "--db-path", "/app/data/budgeteur.db", \
  "--log-path", "/app/data/debug.log", \
  "-a", "0.0.0.0", \
  "-p", "8080" ]
