# syntax=docker/dockerfile:1
FROM golang:1.25.3-bookworm AS go-build
WORKDIR /src
RUN --mount=type=secret,id=proxy_ca,target=/run/proxy-ca.pem \
    if [ -f /run/proxy-ca.pem ]; then export GIT_SSL_CAINFO=/run/proxy-ca.pem; fi; \
    git init && git remote add origin https://github.com/amnezia-vpn/amneziawg-go.git && git fetch --depth=1 origin b5928efb6ca19f0153958460c3d141f04abc5c2e && git checkout --detach FETCH_HEAD
RUN --mount=type=secret,id=proxy_ca,target=/run/proxy-ca.pem \
    if [ -f /run/proxy-ca.pem ]; then export SSL_CERT_FILE=/run/proxy-ca.pem; fi; \
    GOMAXPROCS=2 CGO_ENABLED=0 go build -p 2 -trimpath -o /amneziawg-go .

FROM golang:1.25.3-bookworm AS tools-build
WORKDIR /src
RUN --mount=type=secret,id=proxy_ca,target=/run/proxy-ca.pem \
    if [ -f /run/proxy-ca.pem ]; then export GIT_SSL_CAINFO=/run/proxy-ca.pem; fi; \
    git init && git remote add origin https://github.com/amnezia-vpn/amneziawg-tools.git && git fetch --depth=1 origin ee0f0a9aa34ff0a0da4b3433b9512781cfe02843 && git checkout --detach FETCH_HEAD && make -C src -j2

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends iproute2 iptables && rm -rf /var/lib/apt/lists/*
COPY --from=go-build /amneziawg-go /usr/local/bin/amneziawg-go
COPY --from=tools-build /src/src/wg /usr/local/bin/awg
COPY --from=go-build /src/LICENSE /usr/share/licenses/amneziawg-go/LICENSE
COPY --from=tools-build /src/COPYING /usr/share/licenses/amneziawg-tools/COPYING
COPY entrypoint.sh /usr/local/bin/start-awg
RUN chmod 755 /usr/local/bin/start-awg
ENTRYPOINT ["/usr/local/bin/start-awg"]
