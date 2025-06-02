FROM rust:1.85.1-alpine3.21 AS build

RUN apk update
RUN apk add musl-dev

WORKDIR /build

COPY Cargo.toml ./Cargo.toml 
COPY Cargo.lock ./Cargo.lock
COPY templates ./templates 
COPY src/ ./src

RUN cargo build --verbose --profile release

#=============================================================================#

FROM alpine:3.21 AS deploy

WORKDIR /app

COPY --from=build /build/target/release/server /app/server
COPY --from=build /build/target/release/reset_password /app/reset_password
COPY templates ./templates 
COPY static/ ./static

EXPOSE 3000

CMD [ "/app/server", "--db-path", "app.db", "-a", "0.0.0.0" ]
