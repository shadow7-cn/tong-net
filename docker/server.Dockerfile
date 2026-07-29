FROM node:20-bookworm-slim AS web-builder
WORKDIR /source
COPY package.json package-lock.json ./
COPY apps/server-web/package.json apps/server-web/package.json
COPY apps/desktop/package.json apps/desktop/package.json
RUN npm ci
COPY apps/server-web apps/server-web
RUN npm run build -w server-web

FROM rust:1-bookworm AS rust-builder
WORKDIR /source/apps/server
COPY apps/server/Cargo.toml apps/server/Cargo.lock ./
COPY apps/server/src ./src
COPY apps/server/tests ./tests
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/source/apps/server/target \
    cargo build --release --locked \
    && cp /source/apps/server/target/release/tong-net-server /tmp/tong-net-server

FROM debian:bookworm-slim AS easytier
ARG TARGETARCH
ARG EASYTIER_VERSION=2.6.4
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl unzip \
    && case "$TARGETARCH" in \
      amd64) ARCH=x86_64; SHA256=61b659eaedba658fa66fe47d17e1426cdd77e5d02fa15fed447bb4357c09dfd6 ;; \
      arm64) ARCH=aarch64; SHA256=f533ec25a7ea714e09f645615012200278058525795cc3bb690ff011aec1a70f ;; \
      *) echo "不支持的 Docker 架构: $TARGETARCH" >&2; exit 1 ;; \
    esac \
    && curl -fsSL -o /tmp/easytier.zip "https://github.com/EasyTier/EasyTier/releases/download/v${EASYTIER_VERSION}/easytier-linux-${ARCH}-v${EASYTIER_VERSION}.zip" \
    && echo "${SHA256}  /tmp/easytier.zip" | sha256sum -c - \
    && unzip -q /tmp/easytier.zip -d /tmp/easytier \
    && find /tmp/easytier -name easytier-core -type f -exec cp {} /usr/local/bin/easytier-core \; \
    && find /tmp/easytier -name easytier-cli -type f -exec cp {} /usr/local/bin/easytier-cli \; \
    && chmod 0755 /usr/local/bin/easytier-core /usr/local/bin/easytier-cli \
    && curl -fsSL -o /EASYTIER-LICENSE "https://raw.githubusercontent.com/EasyTier/EasyTier/v${EASYTIER_VERSION}/LICENSE"

FROM debian:bookworm-slim
LABEL org.opencontainers.image.title="同网互通组网服务" \
      org.opencontainers.image.description="同网互通公共/私有 EasyTier 组网服务" \
      org.opencontainers.image.source="https://github.com/shadow7-cn/tong-net" \
      org.opencontainers.image.licenses="MIT AND LGPL-3.0"
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl tzdata \
    && rm -rf /var/lib/apt/lists/*
COPY --from=rust-builder /tmp/tong-net-server /usr/local/bin/tong-net-server
COPY --from=web-builder /source/apps/server-web/dist /opt/tong-net/web
COPY --from=easytier /usr/local/bin/easytier-core /usr/local/bin/easytier-core
COPY --from=easytier /usr/local/bin/easytier-cli /usr/local/bin/easytier-cli
COPY --from=easytier /EASYTIER-LICENSE /usr/share/licenses/easytier/LICENSE
COPY LICENSE /usr/share/licenses/tong-net/LICENSE

ENV TONGNET_WEB_PORT=17280 \
    TONGNET_EASYTIER_PORT=11010 \
    TONGNET_INTERNAL_EASYTIER_HOST=127.0.0.1 \
    TONGNET_DATA_DIR=/data \
    TONGNET_WEB_DIR=/opt/tong-net/web \
    RUST_LOG=info \
    TZ=Asia/Shanghai

VOLUME ["/data"]
EXPOSE 17280/tcp 11010/tcp 11010/udp
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
  CMD curl -fsS "http://127.0.0.1:${TONGNET_WEB_PORT}/healthz" >/dev/null || exit 1
ENTRYPOINT ["/usr/local/bin/tong-net-server"]
