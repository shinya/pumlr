#!/usr/bin/env bash
# 比較開発ループ用スクリプト。
# tests/fixtures/*.puml を Java PlantUML (正解) と pumlr の両方でレンダリングし、
# compare/ に <name>_ref.png / <name>_ref.svg / <name>_pumlr.png / <name>_pumlr.svg を出力する。
#
# 使い方:
#   scripts/compare.sh              # 全 fixture を比較
#   scripts/compare.sh simple_activity  # 指定 fixture のみ

set -euo pipefail
cd "$(dirname "$0")/.."

OUT=compare
mkdir -p "$OUT"

cargo build --example render --quiet

fixtures=(tests/fixtures/*.puml)
if [[ $# -ge 1 ]]; then
    fixtures=()
    for name in "$@"; do
        fixtures+=("tests/fixtures/${name}.puml")
    done
fi

for f in "${fixtures[@]}"; do
    name=$(basename "$f" .puml)

    # 正解: Java PlantUML
    plantuml -tpng -o "$PWD/$OUT" "$f"
    mv "$OUT/$name.png" "$OUT/${name}_ref.png"
    plantuml -tsvg -o "$PWD/$OUT" "$f"
    mv "$OUT/$name.svg" "$OUT/${name}_ref.svg"

    # pumlr
    if ./target/debug/examples/render "$f" "$OUT/${name}_pumlr.svg" > /dev/null; then
        rsvg-convert -o "$OUT/${name}_pumlr.png" "$OUT/${name}_pumlr.svg"
        echo "OK   $name"
    else
        echo "FAIL $name (pumlr render error)"
    fi
done

echo "outputs in $OUT/"
