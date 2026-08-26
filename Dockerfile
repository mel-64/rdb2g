FROM rust:alpine AS builder
WORKDIR /app
RUN apk --no-cache add ca-certificates
ADD . .
RUN --mount=type=cache,target=/app/target \
    --mount=type=cache,target=/var/cache/cargo \
    cargo build --release && cp /app/target/release/rdb2g /

FROM scratch
COPY --from=builder --parents /rdb2g /etc/ssl/certs /
USER 1000:1000
ENTRYPOINT ["/rdb2g"]
