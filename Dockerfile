FROM rust:1.85.1-alpine3.21 AS build

RUN apk update
RUN apk add musl-dev

WORKDIR /build

COPY Cargo.toml ./Cargo.toml 
COPY Cargo.lock ./Cargo.lock
COPY templates ./templates 
COPY src/ ./src

# TODO: Only build server and reset_password binaries
RUN cargo build --verbose --profile release

#=============================================================================#

FROM alpine:3.21 AS deploy

WORKDIR /app

COPY --from=build /build/target/release/server /usr/local/bin/server
COPY --from=build /build/target/release/reset_password /usr/local/bin/reset_password
COPY templates ./templates 
COPY static/ ./static

EXPOSE 3000

CMD [ "server", "--db-path", "app.db", "-a", "0.0.0.0" ]
