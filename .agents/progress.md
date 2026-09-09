# Active Task Progress

- Objective: ドットツール開発指示を、1bitベクター変換ツールではなく、Aseprite型のドット絵練習ツールを目指す方向へ修正する。
- Status: Complete.
- Completed work: `AGENTS.md`からベクター正、1bit、Lua等の出力API前提を除き、ラスターピクセル、パレット、レイヤー／フレーム、練習ループ、等倍確認、PNG／アニメーション／スプライトシートを中心とする指示へ変更した。`.agents/dot-tool-development.md`も同じ方向へ全面改訂した。
- Key decisions: `資料/ドット絵ツールの知見.md`は旧アプローチの参考資料として残すが、プロダクト方向の根拠にはしない。`資料/ドット絵練習ガイド.md`と`資料/Asepriteで始めるドット絵練習ガイド.md`を主要根拠にする。
- Files changed: `AGENTS.md`、`.agents/dot-tool-development.md`、`.agents/progress.md`。
- Verification: `git diff --check`成功。`AGENTS.md`と開発ガイドを検索し、ベクター、1bit、Playdate、Luaは「現方針ではない」と示す箇所以外に残っていないことを確認した。
- Blockers or open questions: なし。
- Exact next steps: なし。
