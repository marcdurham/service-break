# Frontend image: Trunk builds the Yew app to static wasm/JS, nginx serves it
# and proxies /api to the backend service. Build from the repo root:
#   docker build -f frontend/Dockerfile .

FROM rust:1.88-slim-bookworm AS builder
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && rustup target add wasm32-unknown-unknown
# Prebuilt trunk binary; cargo-installing it would rebuild it from source.
ARG TRUNK_VERSION=v0.21.14
ARG TARGETARCH
RUN case "$TARGETARCH" in \
      arm64) triple=aarch64-unknown-linux-gnu ;; \
      *) triple=x86_64-unknown-linux-gnu ;; \
    esac \
    && curl -fsSL "https://github.com/trunk-rs/trunk/releases/download/${TRUNK_VERSION}/trunk-${triple}.tar.gz" \
       | tar -xz -C /usr/local/bin trunk
WORKDIR /app
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cd frontend && trunk build --release

FROM nginx:alpine
COPY frontend/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/frontend/dist /usr/share/nginx/html
EXPOSE 80
