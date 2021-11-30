FROM rust:alpine AS builder

WORKDIR /work

RUN apk update && apk add musl-dev

COPY src ./src

COPY Cargo.toml Cargo.lock ./

RUN cargo build --release

FROM alpine:latest

WORKDIR /work

COPY --from=builder ./work/target/release/chi-tg-inline-rs ./

EXPOSE 8080

CMD ["./chi-tg-inline-rs"]