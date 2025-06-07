FROM rust:1.85.1-alpine3.21 AS build

RUN apk update
RUN apk add --no-cache musl-dev 

WORKDIR /build

COPY Cargo.toml ./Cargo.toml 
COPY Cargo.lock ./Cargo.lock
COPY templates ./templates 
COPY src/ ./src

RUN cargo build --verbose --release --bin server --bin reset_password

#==============================================================================#
 
FROM alpine:3.21 AS tailwind

WORKDIR /build

COPY templates/ /build/templates

RUN apk update
RUN apk add --no-cache curl libgcc libstdc++
RUN curl -sL https://github.com/tailwindlabs/tailwindcss/releases/download/v4.1.8/tailwindcss-linux-x64-musl -o tailwindcss && \
  chmod +x tailwindcss && \
  mv tailwindcss /usr/bin

RUN tailwindcss --input templates/source.css --output static/main.css --minify

#==============================================================================#

FROM alpine:3.21 AS deploy

WORKDIR /app

COPY templates ./templates 
COPY static/ ./static
COPY --from=tailwind /build/static/main.css /app/static/main.css
COPY --from=build /build/target/release/server /usr/local/bin/server
COPY --from=build /build/target/release/reset_password /usr/local/bin/reset_password

EXPOSE 3000

CMD [ "server", "--db-path", "/app/data/budgeteur.db", "-a", "0.0.0.0", "-p", "8080" ]
