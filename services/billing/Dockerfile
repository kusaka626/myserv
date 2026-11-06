# syntax=docker/dockerfile:1
FROM rust:1.99.1-bookworm AS build
WORKDIR /src
COPY . .
RUN --mount=type=secret,id=proxy_ca,target=/run/proxy-ca.pem \
    if [ -f /run/proxy-ca.pem ]; then export CARGO_HTTP_CAINFO=/run/proxy-ca.pem; fi; \
    cargo build --locked --release -p myserv-billing -j2
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/* && useradd --uid 10001 --create-home app && install -d -o app -g app -m 700 /data
COPY --from=build /src/target/release/myserv-billing /usr/local/bin/myserv-billing
USER 10001
ENV MYSERV_DATABASE=/data/orders.db MYSERV_LISTEN=0.0.0.0:8090
VOLUME /data
EXPOSE 8090
ENTRYPOINT ["myserv-billing"]
