#!/usr/bin/env bash
set -euo pipefail
test "$(id -u)" = 0
test -d /run/systemd/system
. /etc/os-release
test "$ID" = ubuntu
test "$VERSION_ID" = 24.04
case "$(uname -m)" in x86_64|aarch64) ;; *) exit 10;; esac
awk '/MemTotal/ { if ($2 < 1900000) exit 1 }' /proc/meminfo
test "$(df -Pk /var | awk 'NR==2 {print $4}')" -ge 8000000
command -v python3 >/dev/null
if [ ! -f /var/lib/myserv/owned ]; then
    if command -v k3s >/dev/null || command -v kubeadm >/dev/null; then exit 11; fi
    if ss -Hlnut | awk '{print $5}' | grep -Eq ':(443|51820)$'; then exit 12; fi
    # Do not add privileged networking to a machine already hosting containers.
    if command -v docker >/dev/null; then test -z "$(docker ps -aq)"; fi
fi
printf 'Ubuntu 24.04; root; systemd; >=2 GB RAM; >=8 GB free; %s\n' "$(uname -m)"
