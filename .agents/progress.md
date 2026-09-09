# Active Task Progress

- Objective: デスクトップUIの日本語文字化けを修正する。
- Status: Complete; 起動時にOS標準の日本語フォントをeguiへ登録するよう修正した。
- Completed work: macOSではHiragino、WindowsではYu Gothic／Meiryo／MS Gothic、LinuxではNoto CJKを探索し、見つかったフォントをProportionalとMonospace両方の最優先フォールバックへ登録。未検出時はステータス欄で通知。macOSの分解Unicodeファイル名にも対応。
- Key decisions and reasons: フォントの再配布・ライセンス追加を避けつつ両対象OSで日本語を表示するため、各OS同梱フォントを実行時に利用する。既定英数字フォントより先に登録し、日本語と句読点を一貫して描画する。
- Files changed: `apps/desktop/src/main.rs`、`AGENTS.md`、`.agents/progress.md`。
- Verification performed and results: 現在のmacOSで日本語フォント検出テスト成功。workspace全10 tests成功。fmt、Clippy警告ゼロ、git diff検査も成功。
- Blockers or open questions: Windowsのフォントパスは標準搭載候補を実装したが、このmacOS環境では実画面確認不可。
- Exact next steps: `cargo run -p dotted-desktop`で再起動し、ホーム、観察入力、ステータスの日本語を目視確認する。WindowsではCI後に実機表示を確認する。
