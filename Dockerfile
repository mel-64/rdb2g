FROM rust:alpine AS builder
WORKDIR /app
RUN apk --no-cache add ca-certificates
ADD . .
RUN --mount=type=cache,target=/app/target \
    --mount=type=cache,target=/var/cache/cargo \
    cargo build --release && cp /app/target/release/rdb2g /

FROM scratch
COPY --from=builder /rdb2g /rdb2g
COPY --from=builder /etc/ssl/certs /etc/ssl/certs 
ENTRYPOINT ["/rdb2g"]
