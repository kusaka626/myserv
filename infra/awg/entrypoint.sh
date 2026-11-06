#!/bin/sh
set -eu
cleanup() {
    iptables -t nat -D POSTROUTING -s 10.78.0.0/24 -o "$uplink" -j MASQUERADE 2>/dev/null || true
    iptables -D FORWARD -i awg0 -j ACCEPT 2>/dev/null || true
    iptables -D FORWARD -o awg0 -m conntrack --ctstate RELATED,ESTABLISHED -j ACCEPT 2>/dev/null || true
    ip link del awg0 2>/dev/null || true
    kill "$pid" 2>/dev/null || true
}
uplink=$(ip -4 route show default | awk 'NR==1 {print $5}')
test -n "$uplink"
amneziawg-go -f awg0 &
pid=$!
trap cleanup EXIT
trap 'exit 0' TERM INT
n=0
until ip link show awg0 >/dev/null 2>&1; do
    n=$((n+1)); test "$n" -lt 30; sleep 1
done
awg setconf awg0 /etc/myserv/awg.conf
ip address add 10.78.0.1/24 dev awg0
ip link set mtu 1280 up dev awg0
iptables -t nat -C POSTROUTING -s 10.78.0.0/24 -o "$uplink" -j MASQUERADE 2>/dev/null || iptables -t nat -A POSTROUTING -s 10.78.0.0/24 -o "$uplink" -j MASQUERADE
iptables -C FORWARD -i awg0 -j ACCEPT 2>/dev/null || iptables -I FORWARD -i awg0 -j ACCEPT
iptables -C FORWARD -o awg0 -m conntrack --ctstate RELATED,ESTABLISHED -j ACCEPT 2>/dev/null || iptables -I FORWARD -o awg0 -m conntrack --ctstate RELATED,ESTABLISHED -j ACCEPT
wait "$pid"
