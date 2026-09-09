# Active Task Progress

- Objective: `資料/ドット絵スキル向上ツール仕様.md`を実装可能なプログラム設計へ具体化する。
- Status: Complete.
- Completed work: 仕様、開発ガイド、既存進捗、リポジトリ状態を確認し、Phase 1を実装可能な設計へ具体化した。技術判断、Cargo workspaceと依存方向、ドメイン型、編集コマンドとUndo境界、非破壊診断、画面遷移、`.dotted`保存形式、自動保存、エラー処理、テスト、8段階の実装順序、性能目標を設計文書へ記載した。開発ガイドと`AGENTS.md`へ恒久的な判断を同期した。
- Key decisions: 単一デスクトップアプリをCargo workspaceで構成し、`pixel_core`、`training`、`project_io`をUI非依存にする。MVPでは単一フレーム・単一spriteレイヤーにUIを制限するが、永続モデルは将来の複数レイヤー／フレームを自然に追加できる構造にする。`.dotted`はZIP＋JSON＋RGBA PNG、履歴は再構築可能なSQLite索引とする。CopyからMemoryへの遷移APIはcopyピクセルを受け取らず、独立した透明グリッドを生成する。
- Files changed: `資料/Dotted Training Studioプログラム設計.md`、`.agents/dot-tool-development.md`、`AGENTS.md`、`.agents/progress.md`。
- Verification: `git diff --check`成功。仕様のMVP受け入れ条件を設計のテスト項目へ対応付け、白紙Memory、非破壊診断、1ジェスチャー1 Undo、色数超過、保存往復、未完了再開を確認した。コード未実装のためビルド／テストは対象外。
- Blockers or open questions: なし。未決定事項はMVP推奨判断として設計文書に明記する。
- Exact next steps: 実装へ進む場合は設計の手順1に従い、Cargo workspace、`pixel_core`の基本型、ASCII fixture、CIを作成する。併せて対応OS下限、ファイル上限、日時型、eguiテクスチャ更新方式を短いADRで確定する。
