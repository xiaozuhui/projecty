#!/usr/bin/env bash
# 编译项目 Docker 镜像(后端 API + 前端静态产物合并为单镜像)。
# tag 名与 deploy/docker/compose.yml 引用的 image 保持一致(projecty-backend)。
#
# 用法:
#   ./build.sh                # 构建单镜像(backend,含前端静态产物),打 latest tag
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

for name in backend; do
  echo "==> 构建 projecty-${name}:${TAG}"
  docker build \
    "${platform_args[@]+"${platform_args[@]}"}" \
    --file "deploy/docker/${name}.Dockerfile" \
    --tag "projecty-${name}:${TAG}" \
    .
done

echo "==> 构建完成:"
# 只列本次构建的镜像;旧架构遗留的 projecty-frontend 可用 docker rmi 清理。
docker image ls --filter="reference=projecty-backend:${TAG}"
