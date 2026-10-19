#!/usr/bin/env bash
#
# 判断「服务端相关输入」相对某个基准提交/标签是否发生变化。
#
# 用途：CI 里没有改动就不要跑服务端测试与镜像构建（一次冷启动的多架构构建约 20 分钟）。
#
# 用法：
#   scripts/server-changed.sh <base-ref> [head-ref]      # head 默认 HEAD
#
# 输出：
#   stdout 最后一行 `true` / `false`（同时写入 $GITHUB_OUTPUT 的 changed=）。
#   人类可读的原因写到 stderr。
#
# 判定规则（宁可多跑，不可漏跑）：
#   - crates/server/**、crates/sync-proto/**、.dockerignore、docker-compose*.yml
#     → 只要有文件变动就算变更；
#   - 各 Cargo.toml / Cargo.lock → **只改版本号不算变更**。
#     发布时三个 workspace 包会一起 bump 版本，若照单全收，纯客户端改动也会
#     触发服务端重建；而依赖增减必然伴随 name/checksum/dependencies 等行的变动，
#     所以「所有增删行都是 `version = "…"`」可以安全地视为无关噪声；
#   - 基准无法解析（新分支的全零 SHA、首次发布没有上一个 tag）→ 保守判为变更。
#
set -euo pipefail

base="${1:-}"
head="${2:-HEAD}"
head="${head:-HEAD}"

say() { printf '%s\n' "$*" >&2; }

emit() {
  local value="$1"
  printf '%s\n' "$value"
  if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
    printf 'changed=%s\n' "$value" >>"$GITHUB_OUTPUT"
  fi
}

# 基准不可用时保守执行完整流程。
if [[ -z "$base" ]] || [[ "$base" =~ ^0+$ ]]; then
  say "server-changed: 没有可用的比对基准（base='${base}'）→ 视为有变更"
  emit true
  exit 0
fi

if ! git rev-parse --verify -q "${base}^{commit}" >/dev/null; then
  say "server-changed: 基准 '${base}' 在本地不可解析 → 视为有变更"
  emit true
  exit 0
fi
if ! git rev-parse --verify -q "${head}^{commit}" >/dev/null; then
  say "server-changed: head '${head}' 不可解析 → 视为有变更"
  emit true
  exit 0
fi

# 只看「增删行」是否全部是版本号；有其它改动即为真实变更。
only_version_churn() {
  local file="$1"
  local other
  other="$(git diff --unified=0 "$base" "$head" -- "$file" \
    | grep -E '^[+-]' \
    | grep -vE '^(\+\+\+|---)' \
    | grep -vE '^[+-]version = "' || true)"
  [[ -z "$other" ]]
}

changed_file=""
while IFS= read -r file; do
  [[ -z "$file" ]] && continue
  case "$file" in
    # Cargo.toml / Cargo.lock 先判：允许纯版本号 bump。
    Cargo.toml|Cargo.lock|crates/*/Cargo.toml)
      if only_version_churn "$file"; then
        continue
      fi
      changed_file="$file"
      ;;
    crates/server/*|crates/sync-proto/*|.dockerignore|docker-compose.yml|docker-compose.tls.yml)
      changed_file="$file"
      ;;
  esac
  [[ -n "$changed_file" ]] && break
done < <(git diff --name-only "$base" "$head")

if [[ -n "$changed_file" ]]; then
  say "server-changed: ${base}..${head} 命中服务端输入变更（${changed_file}）→ 需要构建/测试"
  emit true
else
  say "server-changed: ${base}..${head} 无服务端输入变更 → 跳过"
  emit false
fi
