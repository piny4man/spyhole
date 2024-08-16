FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ARG RUST_LOG
ARG APP_PORT
ARG DATABASE_URL

COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
ENV RUST_LOG=$RUST_LOG
ENV APP_PORT=$APP_PORT
ENV DATABASE_URL=$DATABASE_URL

RUN echo $DATABASE_URL
RUN cargo build --release

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/spyhole /usr/local/bin/spyhole

EXPOSE 8080
ENTRYPOINT ["spyhole"]
