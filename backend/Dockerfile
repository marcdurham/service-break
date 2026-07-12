# Backend API image. Build from the repo root (the cargo workspace):
#   docker build -f backend/Dockerfile .
# Migrations and seed.sql are embedded in the binary at compile time
# (sqlx::migrate! / include_str!), so the runtime stage only needs the binary.

FROM rust:1.88-slim-bookworm AS builder
WORKDIR /app
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release -p backend \
    && cp target/release/backend /usr/local/bin/backend

FROM debian:bookworm-slim
# CA certificates for outbound HTTPS (Nominatim geocoding).
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/bin/backend /usr/local/bin/backend
ENV BIND_ADDR=0.0.0.0:8081
EXPOSE 8081
CMD ["backend"]
