#!/usr/bin/env bash
set -euo pipefail

# --- 兜底初始化（防止 unbound variable） ---
: "${BASE_URL:=}"
: "${PORT:=}"
: "${NAME:=tgstate}"
: "${VOL:=tgstate-data}"

IMG="ghcr.io/buyi06/tgstate-python@sha256:e897ce4c2b61e48a13ef0ec025dfd80148ed8669d75f688a1a8d81036fe116e5"

# --- 端口交互逻辑 ---
if [[ -z "${PORT}" ]]; then
  if [[ -t 0 ]]; then
    read -r -p "请输入端口 [默认 8000]: " input_port < /dev/tty
    PORT="${input_port:-8000}"
  else
    PORT="8000"
  fi
fi

# 端口合法性兜底 (纯数字且在 1-65535 之间)
if ! [[ "$PORT" =~ ^[0-9]+$ ]] || ((PORT < 1 || PORT > 65535)); then
  echo "端口 '$PORT' 非法，强制回退到 8000" >&2
  PORT=8000
fi

# --- BASE_URL 交互逻辑 ---
if [[ -z "${BASE_URL}" ]]; then
  if [[ -t 0 ]]; then
    read -r -p "请输入 BASE_URL (留空自动使用公网IP): " input_url < /dev/tty
    BASE_URL="${input_url:-}"
  fi
  
  # 如果仍为空，尝试获取公网 IP
  if [[ -z "${BASE_URL}" ]]; then
    IP="$(curl -s4 api.ipify.org || hostname -I | awk '{print $1}' || echo '127.0.0.1')"
    BASE_URL="http://${IP}:${PORT}"
  fi
fi

if ! command -v docker >/dev/null 2>&1; then
  echo "docker 未安装或不可用" >&2
  exit 1
fi

docker rm -f "${NAME}" >/dev/null 2>&1 || true
docker volume inspect "${VOL}" >/dev/null 2>&1 || docker volume create "${VOL}" >/dev/null
docker pull "${IMG}"

docker run -d \
  --name "${NAME}" \
  --restart unless-stopped \
  -p "${PORT}:8000" \
  -v "${VOL}:/app/data" \
  -e "BASE_URL=${BASE_URL}" \
  "${IMG}" >/dev/null

echo "tgState 已启动"
echo "访问地址：${BASE_URL}"

