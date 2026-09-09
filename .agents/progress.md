# Active Task Progress

- Objective: 仕様・設計を9観点（型・責務、依存、Undo、形式、競合、Memory、egui、MVP、テスト）で精査・修正する。
- Status: In progress; 仕様・設計の修正と最終構造レビュー完了、文書検証中。
- Completed work: 型所有と最小MVPモデル、独立パレット、原子的な段階遷移、Undo不変条件、保存v1検証、単一I/Oワーカーと世代判定、復旧保証、egui入力描画、AC-01〜12のテスト対応表を反映。開発ガイドとAGENTSも同期。
- Key decisions: 復旧は最後の書込み成功snapshotまで。1 Project＝1セッション、確定版編集分岐・タグ等は延期。半透明RGBAを保持。1×は物理ピクセル。実装はまだ存在しないため、テスト表は実装時の契約として明記。
- Files changed: 資料/Dotted Training Studioプログラム設計.md、資料/ドット絵スキル向上ツール仕様.md、AGENTS.md、.agents/dot-tool-development.md、.agents/progress.md。
- Verification: egui InputState/Event/Context/ColorImage、Rust renameの一次資料確認。関連文書を横断再読し、重複テスト一覧を整理、逆dev依存・型名・保存範囲の残存矛盾を修正。
- Blockers or open questions: 作業上の阻害なし。OS最低版・依存版・DPI/IME/ファイル置換は基盤スパイクの残存検証事項として明記。
- Exact next steps: Markdown構造・ローカルリンク・AC対応・git diff --checkを検証し、完了記録と結果報告。
