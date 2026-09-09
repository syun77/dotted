# Active Task Progress

- Objective: 開発規模・工数・費用の見積もりを`資料/見積もり.md`として保存する。
- Status: Complete.
- Completed work: 仕様、Phase 1プログラム設計、開発ガイド、リポジトリ状態を確認した。Phase 1を機能群別に分解した人月・費用、体制別の期間、Phase 2〜4の概算、別途費用、推奨スパイク、見積もり精度を`資料/見積もり.md`へ保存した。
- Key decisions: 見積もりは1人月=20人日、税別の開発単価を80万〜120万円/人月とする。Phase 1は製品MVPとして6〜9人月を基準にし、不確実性予備を費用レンジへ含める。ユーザー調査の募集謝礼、コード署名、配布費、機材、継続保守は別費用として扱う。
- Files changed: `資料/見積もり.md`、`.agents/progress.md`。
- Verification: 対象2文書のMVP範囲、受け入れ条件、8段階の実装順序、Phase 2〜4の範囲を照合し、見積もり文書へ反映した。生成したMarkdownを再読し、`git diff --check`成功を確認した。
- Blockers or open questions: 費用は担当者のRust/egui経験、UIデザイン品質、Windows/macOS両方の実機QA、配布方法により変動する。正式見積もりにはこれらの確定が必要。
- Exact next steps: 実装開始時はPhase 1の最初の2週間を基盤・技術スパイクに充て、その実績速度で残工数を再見積もりする。
