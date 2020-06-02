FROM rust:slim AS build

ENV USER=test

RUN rustup target add x86_64-unknown-linux-musl \
    && apt-get update \
    && apt-get install -y make musl-tools perl \
    && cargo new quiz

WORKDIR /quiz

COPY Cargo.toml Cargo.lock /quiz/

RUN cargo build --release --target x86_64-unknown-linux-musl

COPY src /quiz/src/

RUN touch src/main.rs \
    && cargo build --release --target x86_64-unknown-linux-musl

FROM alpine:3.11

RUN apk add --no-cache openssl

COPY --from=build /quiz/target/x86_64-unknown-linux-musl/release/foodtech-quiz /usr/local/bin/

CMD ["foodtech-quiz"]
