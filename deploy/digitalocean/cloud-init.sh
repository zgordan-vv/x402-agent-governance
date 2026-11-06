#!/usr/bin/env bash
set -euo pipefail

# DigitalOcean Ubuntu 24.04 droplet bootstrap. This installs the runtime only;
# application secrets are injected later through the deployment environment.
export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y ca-certificates curl git ufw
install -m 0755 -d /etc/apt/keyrings
curl -fsSL https://download.docker.com/linux/ubuntu/gpg -o /etc/apt/keyrings/docker.asc
chmod a+r /etc/apt/keyrings/docker.asc
echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/ubuntu $(. /etc/os-release && echo \"$VERSION_CODENAME\") stable" > /etc/apt/sources.list.d/docker.list
apt-get update
apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
systemctl enable --now docker

install -d -m 0750 /opt/x402-agent-governance
useradd --system --home /opt/x402-agent-governance --shell /usr/sbin/nologin x402 || true
chown x402:x402 /opt/x402-agent-governance

ufw default deny incoming
ufw default allow outgoing
ufw allow OpenSSH
ufw allow 80/tcp
ufw allow 443/tcp
ufw --force enable
echo 'Cloud host bootstrap complete. Deploy the immutable application image separately.'
