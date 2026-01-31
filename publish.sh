#!/usr/bin/env bash

set -e   # 任一命令失败立即退出

ROOT_DIR=$(pwd)

echo "📦 Rust workspace 单个项目发布脚本"
echo ""

# -----------------------------
# 发现 workspace crates
# -----------------------------
echo "📚 获取 workspace crate 列表..."

# 构建 workspace 包的 {name, manifest_path} JSON 流
PACKAGES_JSON=$(cargo metadata --no-deps --format-version=1 \
    | jq -c '.packages[] | select(.source == null) | {name: .name, path: .manifest_path}')

echo "🧩 发现以下 crates："
# 打印列表以便参考
echo "$PACKAGES_JSON" | jq -r '"- " + .name'
echo ""

# -----------------------------
# 用户输入
# -----------------------------
read -p "请输入要发布的工程名称: " TARGET_NAME

if [ -z "$TARGET_NAME" ]; then
    echo "❌ 未输入工程名称，退出"
    exit 1
fi

# -----------------------------
# 定位目标 crate
# -----------------------------
# 使用 jq 选出匹配的 manifest_path
MANIFEST=$(echo "$PACKAGES_JSON" | jq -r --arg name "$TARGET_NAME" 'select(.name == $name) | .path')

if [ -z "$MANIFEST" ]; then
    echo "❌ 未找到名为 '$TARGET_NAME' 的工程"
    exit 1
fi

DIR=$(dirname "$MANIFEST")

echo ""
echo "=============================="
echo "📦 目标 crate: $TARGET_NAME"
echo "📁 路径 : $DIR"
echo "=============================="

cd "$DIR"

echo "🧪 执行 dry-run..."

# 失败时捕获标准输出与标准错误
if ! OUTPUT=$(cargo publish --dry-run 2>&1); then
    echo "❌ dry-run 失败：$TARGET_NAME"
    echo "   👉 错误信息："
    echo "$OUTPUT"
    exit 1
fi

echo "✔ dry-run 成功：$TARGET_NAME"

# 不再二次确认，直接发布
echo "🚀 正在发布 $TARGET_NAME ..."

if ! OUTPUT=$(cargo publish 2>&1); then
    echo "❌ 发布失败：$TARGET_NAME"
    echo "   👉 错误信息："
    echo "$OUTPUT"
    exit 1
fi

echo "✅ 发布成功：$TARGET_NAME"

cd "$ROOT_DIR"
