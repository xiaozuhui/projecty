#!/usr/bin/env bash
# 编译前后端 Docker 镜像。
# tag 名与 deploy/docker/compose.yml 引用的 image 保持一致(projecty-backend / projecty-frontend)。
#
# 用法:
#   ./build.sh                # 编译 backend + frontend,打 latest tag(compose 使用的就是它)
#   ./build.sh v1.0.0         # 以指定 tag 编译,用于发布留档
#
# 可选环境变量:
#   DOCKER_PLATFORM=linux/amd64   # 在 Apple Silicon 上为 x86 服务器交叉编译时设置
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
TAG="${1:-latest}"

command -v docker >/dev/null 2>&1 || {
  echo "错误:未找到 docker 命令" >&2
  exit 1
}

platform_args=()
if [[ -n "${DOCKER_PLATFORM:-}" ]]; then
  platform_args=(--platform "${DOCKER_PLATFORM}")
fi

cd "$ROOT"

for name in backend frontend; do
  echo "==> 构建 projecty-${name}:${TAG}"
  docker build \
    "${platform_args[@]+"${platform_args[@]}"}" \
    --file "deploy/docker/${name}.Dockerfile" \
    --tag "projecty-${name}:${TAG}" \
    .
done

echo "==> 构建完成:"
docker image ls --filter="reference=projecty-*:${TAG}"
