# Active Task Progress

- Objective: 仕様書・プログラム設計に沿い、指定された15段階でDotted Training Studio Phase 1 MVPを実装する。
- Status: Complete locally; Phase 1 MVPの縦切り実装、リファクタリング、macOS上の自動検証を完了。WindowsはCIジョブを構成済みで、実機操作QAのみ別環境で実施が必要。
- Completed work: Cargo workspaceとmacOS/Windows CI、RGBA/ID/PixelGrid、Pencil/Eraser/Bresenham、1ジェスチャー1 Undo、DPI対応キャンバス座標、物理1×プレビュー、Fill/Eyedropper/Palette、練習状態遷移、独立白紙Memory、`.dotted` ZIP+JSON+PNG+SHA-256保存読込、復旧スナップショット、表示専用診断、Copy/Memory比較、振り返り、SQLite履歴索引、原寸/2/4/8倍PNG出力を実装。READMEとAGENTSを同期。
- Key decisions and reasons: 4 crate境界を維持し、ドメインcrateをUI/OS/SQLiteから分離。完全透明RGBAを正規化し、Memory生成APIへCopy画素を渡さない。`.dotted`内の画素はPNGだけに置き、manifestからPixelGridを除去して正規パスとSHA-256を検証。表示診断と物理等倍は保存データを変更しない。
- Files changed: `Cargo.toml`、`Cargo.lock`、`.github/workflows/ci.yml`、`apps/desktop/`、`crates/pixel_core/`、`crates/training/`、`crates/project_io/`、`README.md`、`AGENTS.md`、`.agents/progress.md`。
- Verification performed and results: `cargo test --workspace`成功（9 tests）、`cargo clippy --workspace --all-targets -- -D warnings`成功、`cargo fmt --check`相当の最終整形成功、`git diff --check`成功。macOSで全crateをコンパイル。WindowsはGitHub Actions matrixへ登録したが、この作業環境では未実行。
- Blockers or open questions: Windows実機でDPI、IME、ファイル置換、ダイアログを操作確認する必要がある。macOSでもGUIの人手による初心者フロー評価は未実施。
- Exact next steps: CI結果を確認し、両OSの手動QA（DPI 1/1.25/1.5/2、枠外ドラッグ、IME、保存置換）を実施する。続いて仕様10.5の異常container・保存競合fixtureを拡充する。
