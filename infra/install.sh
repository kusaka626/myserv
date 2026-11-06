#!/usr/bin/env bash
# Executed over pinned SSH as root. MS_BUNDLE contains base64 JSON, never shell code.
set -Eeuo pipefail
umask 077
exec 3>&1
stage=prepare
myserv_failed() {
    local status=$1 line=$2
    trap - ERR
    printf 'MYSERV_ERROR:install-%s\nMYSERV_DETAIL:%s:%s\n' "$stage" "$line" "$status" >&3
    exit "$status"
}
trap 'myserv_failed "$?" "$LINENO"' ERR
exec 9>/var/lock/myserv-setup.lock
flock -n 9 || { printf 'MYSERV_ERROR:install-locked\n' >&3; exit 1; }
exec >>/var/log/myserv-setup.log 2>&1
stage=state
mkdir -p /var/lib/myserv
chmod 700 /var/lib/myserv
export MS_BUNDLE
python3 - <<'PY'
import base64,json,os,pathlib
new=json.loads(base64.b64decode(os.environ['MS_BUNDLE']))
path=pathlib.Path('/var/lib/myserv/state.json')
if path.exists():
    old=json.loads(path.read_text())
    assert old['binding']==new['binding'], 'Server binding changed; manual recovery required'
    assert old['material']['protocol']==new['material']['protocol'], 'Protocol change requires migration'
else:
    tmp=path.with_suffix('.tmp')
    tmp.write_text(json.dumps(new));tmp.chmod(0o600);tmp.replace(path)
root=path.parent
state=json.loads(path.read_text())
(root/'owned').touch(mode=0o600)
(root/'Dockerfile').write_text(state['awg_dockerfile'])
(root/'entrypoint.sh').write_text(state['awg_entrypoint'])
(root/'deployment.json').write_text(state['manifest'])
protocol=state['material']['protocol']
(root/'protocol').write_text(protocol)
(root/('awg.conf' if protocol=='amnezia_wg' else 'server.json')).write_text(state['material']['server_config'])
PY
unset MS_BUNDLE
cd /var/lib/myserv
protocol=$(cat protocol)
echo MYSERV:packages >&3
stage=packages
export DEBIAN_FRONTEND=noninteractive
apt-get update -q
# unattended-upgrades may own dpkg on a newly rented VPS. Let apt wait safely;
# never delete its lock files or interrupt the other package manager.
apt-get -o DPkg::Lock::Timeout=600 install -y --no-install-recommends docker.io curl ca-certificates iptables openssl
if [ "$protocol" = amnezia_wg ]; then
    # docker.io alone can fall back to the legacy builder, which rejects RUN --mount.
    apt-get -o DPkg::Lock::Timeout=600 install -y --no-install-recommends docker-buildx
    docker buildx version
fi
systemctl enable --now docker
if [ "$protocol" = amnezia_wg ]; then
    stage=tun
    test -c /dev/net/tun
    printf 'net.ipv4.ip_forward=1\n' >/etc/sysctl.d/90-myserv.conf
    sysctl -p /etc/sysctl.d/90-myserv.conf
else
    stage=reality
    domain=$(python3 -c "import json; print(json.load(open('server.json'))['inbounds'][0]['streamSettings']['realitySettings']['serverNames'][0])")
    timeout 20 openssl s_client -connect "$domain:443" -servername "$domain" -tls1_3 -alpn h2 -verify_return_error -verify_hostname "$domain" </dev/null >reality-check.log 2>&1
    grep -q 'ALPN protocol: h2' reality-check.log
fi
echo MYSERV:kubernetes >&3
stage=kubernetes-download
if [ ! -x /usr/local/bin/k3s ]; then
    case "$(uname -m)" in x86_64) arch=amd64; binary=k3s;; aarch64) arch=arm64; binary=k3s-arm64;; *) exit 10;; esac
    version='v1.34.13+k3s1'
    mkdir -p downloads
    (
        cd downloads
        curl --fail --location --retry 3 "https://github.com/k3s-io/k3s/releases/download/$version/$binary" -o "$binary"
        curl --fail --location --retry 3 "https://github.com/k3s-io/k3s/releases/download/$version/sha256sum-$arch.txt" -o checksums.txt
        sha256sum --check --ignore-missing checksums.txt
        install -m 755 "$binary" /usr/local/bin/k3s
    )
fi
stage=kubernetes-start
if [ ! -f /etc/systemd/system/myserv-k3s.service ]; then
cat >/etc/systemd/system/myserv-k3s.service <<'UNIT'
[Unit]
Description=MyServ single-node Kubernetes
After=network-online.target docker.service
Wants=network-online.target
Requires=docker.service
[Service]
Type=notify
ExecStart=/usr/local/bin/k3s server --docker --bind-address=127.0.0.1 --disable=traefik,servicelb,metrics-server,coredns --flannel-backend=host-gw --disable-network-policy --secrets-encryption --write-kubeconfig-mode=600 --kubelet-arg=address=127.0.0.1 --kube-proxy-arg=metrics-bind-address=127.0.0.1 --kube-proxy-arg=healthz-bind-address=127.0.0.1
Restart=always
RestartSec=5
Delegate=yes
KillMode=process
LimitNOFILE=1048576
TasksMax=infinity
[Install]
WantedBy=multi-user.target
UNIT
fi
systemctl daemon-reload
systemctl enable --now myserv-k3s
for n in $(seq 1 90); do
    if k3s kubectl get nodes >/dev/null 2>&1; then break; fi
    sleep 2
done
# Single-node host-gw CNI needs no exposed VXLAN port. Control plane is loopback-only.
k3s kubectl get nodes
echo MYSERV:image >&3
stage=image
if [ "$protocol" = amnezia_wg ]; then
    if ! docker image inspect myserv-awg:3.1.20260828 >/dev/null 2>&1; then
        DOCKER_BUILDKIT=1 docker build -t myserv-awg:3.1.20260828 .
    fi
    config=awg.conf
else
    docker pull ghcr.io/xtls/xray-core:26.10.31
    docker run --rm --user 0 -v /var/lib/myserv/server.json:/config.json:ro ghcr.io/xtls/xray-core:26.10.31 run -test -config /config.json
    config=server.json
fi
echo MYSERV:deploy >&3
stage=deploy
k3s kubectl create namespace myserv --dry-run=client -o yaml | k3s kubectl apply -f -
k3s kubectl -n myserv create secret generic tunnel --from-file="$config" --dry-run=client -o yaml | k3s kubectl apply --server-side -f -
k3s kubectl apply -f deployment.json
# Preserve existing firewall policy; add only the selected VPN port if UFW is active.
if command -v ufw >/dev/null && ufw status | grep -q '^Status: active'; then
    if [ "$protocol" = amnezia_wg ]; then ufw allow 51820/udp; else ufw allow 443/tcp; fi
fi
k3s kubectl -n myserv rollout status deployment/tunnel --timeout=180s
if [ "$protocol" = amnezia_wg ]; then
    k3s kubectl -n myserv exec deployment/tunnel -- awg show awg0 public-key >/dev/null
else
    timeout 5 bash -c 'exec 4<>/dev/tcp/127.0.0.1/443'
fi
echo MYSERV:ready >&3
