FROM rust:1.92.0-alpine3.23 AS build

RUN apk update
RUN apk add --no-cache musl-dev 

WORKDIR /build

COPY Cargo.toml /build/Cargo.toml
COPY Cargo.lock /build/Cargo.lock
COPY templates/ /build/templates/
COPY src/ /build/src/

RUN cargo build --verbose --release --bin server --bin reset_password

#==============================================================================#

FROM alpine:3.23 AS tailwind

RUN apk update
RUN apk add --no-cache curl libgcc libstdc++

WORKDIR /build
COPY templates/ /build/templates
RUN curl -sL https://github.com/tailwindlabs/tailwindcss/releases/download/v4.1.18/tailwindcss-linux-x64-musl -o tailwindcss && \
  chmod +x tailwindcss && \
  ./tailwindcss --input templates/source.css --output static/main.css --minify

#==============================================================================#

FROM alpine:3.23 AS deploy

WORKDIR /app

COPY static/ ./static
COPY --from=tailwind /build/static/main.css /app/static/main.css
COPY --from=build /build/target/release/server /usr/local/bin/server
COPY --from=build /build/target/release/reset_password /usr/local/bin/reset_password

EXPOSE 8080

CMD [ "server", "--db-path", "/app/data/budgeteur.db", \
  "--log-path", "/app/data/debug.log", \
  "-a", "0.0.0.0", \
  "-p", "8080" ]
