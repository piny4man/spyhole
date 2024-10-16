FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ARG RUST_LOG
ARG APP_PORT
ARG DATABASE_PUBLIC_URL

COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
ENV RUST_LOG=$RUST_LOG
ENV APP_PORT=$APP_PORT
ENV DATABASE_URL=$DATABASE_PUBLIC_URL
RUN cargo build --release

FROM debian:bookworm-slim AS runtime
ARG RUST_LOG
ARG APP_PORT
ARG DATABASE_PUBLIC_URL

RUN apt-get update \
  && apt-get install -y ca-certificates \
  && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY static /app/static
COPY templates /app/templates
COPY --from=builder /app/target/release/spyhole /usr/local/bin/spyhole

ENV RUST_LOG=$RUST_LOG
ENV APP_PORT=$APP_PORT
ENV DATABASE_URL=$DATABASE_PUBLIC_URL

EXPOSE $APP_PORT
ENTRYPOINT ["spyhole"]
