# Dotted Training Studio

模写、白紙からの記憶描き、等倍確認、比較、振り返りを一続きで行うRust製デスクトップ・ドット絵練習ツールです。

## 起動

Rust 1.85以降を用意し、リポジトリ直下で実行します。

```sh
cargo run -p dotted-desktop
```

ホームの「16×16・4色・30分の練習を始める」から、同梱参照ですぐに練習できます。編集用プロジェクトは`.dotted`、作品は原寸／2×／4×／8×PNGで保存できます。

## 検証

```sh
cargo fmt --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
