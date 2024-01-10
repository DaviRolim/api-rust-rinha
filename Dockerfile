FROM rust:1.75.0-slim-buster

RUN apt-get update -yqq && apt-get install -yqq cmake g++

WORKDIR /axum

RUN mkdir src; touch src/main.rs

COPY Cargo.toml Cargo.lock ./

RUN cargo fetch

COPY src/ ./src/

RUN cargo build --release

EXPOSE 80

CMD ./target/release/rust-api-rinha-backend