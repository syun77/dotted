# Dotted Training Studio プログラム設計

## 1. 設計の目的

本書は`ドット絵スキル向上ツール仕様.md`のPhase 1を、実装・テストできる単位へ分解する。対象はローカル完結のRustデスクトップアプリである。

最重要の設計判断は、ピクセル編集と練習進行を別のドメインとして扱うことである。エディターは成果物を正確に変更し、練習機能は「模写から記憶描きへ進める」などの段階と成果物の関係を管理する。UIは両者を呼び出して表示するだけにする。

## 2. MVPの技術判断

| 項目 | MVPでの判断 | 理由 |
| --- | --- | --- |
| 対象 | Windows、macOSのデスクトップ | ポインターとキーボード中心の操作を先に検証する |
| UI | `eframe/egui` | Rust内で高速にUIを反復でき、独自キャンバス描画と相性がよい |
| 保存 | ZIPコンテナ＋JSON＋PNG | 単一ファイルの扱いやすさと、人が調査できる内部形式を両立する |
| 参照画像 | プロジェクト内へPNGとして埋め込む | 元ファイルの移動後も練習を再開できる |
| 履歴 | ローカルSQLite | 一覧・絞り込み・未完了再開を、全プロジェクト走査なしで行う |
| 自動保存 | アプリ管理領域の復旧スナップショット | ユーザーの明示保存先を勝手に上書きせず復旧可能にする |
| ピクセル形式 | 非premultiplied RGBA8 | PNGとの往復と色数集計の意味を単純に保つ |
| 非同期処理 | 保存、読込、画像デコード、PNG出力、SQLite処理を単一I/Oワーカー | 描画コマンドの順序と決定性を守る |

ファイル拡張子は`.dotted`とする。形式には`format_version`を持たせるが、開発中の旧形式を維持する互換層は作らない。

## 3. 全体構成

```text
apps/desktop
  UI、入力解釈、描画、ダイアログ、アプリケーション状態
        │
        ├── training
        │     課題、段階遷移、成果物作成規則、振り返り
        │
        ├── pixel_core
        │     ピクセル、ツール、編集コマンド、Undo/Redo、合成
        │
        └── project_io
              .dotted、PNG、参照画像、復旧、履歴インデックス
```

推奨Cargo workspaceは次のとおり。

```text
Cargo.toml
apps/
  desktop/
crates/
  pixel_core/
  training/
  project_io/
```

依存方向は`desktop -> {training, pixel_core, project_io}`、`training -> pixel_core`、`project_io -> {training, pixel_core}`に固定する。`pixel_core`と`training`からUI、ファイルダイアログ、OS API、SQLiteへ依存してはならない。

`Project`と`Artwork`、練習規則は`training`、`RasterDocument`と編集履歴は`pixel_core`が所有する。`project_io`は保存DTOへの変換、検証済みドメインの構築、ファイル操作とSQLiteを担当し、練習段階を進めない。`desktop`がコマンド実行、ワーカー起動、復旧要求を調停する。`training -> project_io`は禁止する。保存用serde型は`project_io`内部に置き、ドメイン型をZIP構造に結合しない。

共有fixtureは当初各crateのテストモジュールへ置く。`test_support` crate、非同期ランタイム、DIコンテナは導入しない。CIでは`cargo metadata`の通常依存とdev依存を検査し、逆依存やドメインからegui／SQLiteへの依存を拒否する。

## 4. ドメインモデル

### 4.1 型の所有者と保存範囲

以下の型表と構造は実装契約であり、Rustの完成コードではない。コレクションは順序が必要なら`Vec`、ID検索だけなら`BTreeMap`を使い、独自の`OrderedMap`は作らない。IDはUUIDの型別newtypeとし、ID生成と現在時刻はdesktopから渡す。OpenGenerationは開く操作ごとのUUID、JobIdはその世代内の単調な番号とする。

| 所有crate | 型 | 契約 |
| --- | --- | --- |
| pixel_core | `CanvasSize`, `PixelGrid`, `Rgba8` | 正の整数寸法、row-major RGBA8、長さはwidth×height。フィールドは非公開、検証付き構築。alpha=0はRGBA(0,0,0,0) |
| pixel_core | `RasterDocument`, `Layer`, `Frame`, `LayerId`, `FrameId` | サイズ、パレット、順序付きレイヤー／フレーム、各レイヤーのFrameId→PixelGrid。MVPは表示あり・opacity=255のSpriteレイヤー1枚、フレーム1枚・duration_ms=100固定 |
| pixel_core | `Palette`, `PaletteEntry`, `PaletteEntryId` | エントリーはIDとRGBA8。2～8個、alpha>0、RGBA重複不可。選択色や色数上限を含めない |
| pixel_core | `EditSession`, `EditCommand`, `EditDelta` | 編集用文書、進行中ジェスチャー、Undo／Redo。練習IDや保存先を知らない |
| training | `ProjectId`, `SessionId`, `ArtworkId`, `ReferenceId` | プロジェクト内の対応する型のIDは一意 |
| training | `Project`, `Artwork`, `PracticeSession`, `Reference` | 練習と成果物の集約。ピクセルはRasterDocumentへ委譲 |
| training | `Constraints`, `ObservationNotes`, `Reflection` | サイズ、initial_palette、color_limit(2～8)、time_limit_ms(正)、背景プリセット。観察はoutline/value/identifying_feature、振り返りはgoal/success/next_try |
| training | `PracticeEvent`, `StageArtifact`, `ReferenceVisibility` | イベントは通し番号・UTC時刻・段階・型付き内容。成果物リンクは段階とArtworkId。参照状態はHidden/Revealed |
| project_io | `ManifestV1`, `ProjectSnapshot`, `ProjectIoError` | 保存DTOと検証、文書全体の不変な保存入力、構造化エラー |
| desktop | `EditorUiState`, `LoadedProject`, `SaveState`, `JobId`, `OpenGeneration` | 入力・表示・履歴所有の調停、保存先、ジョブ識別。保存ドメインに混ぜない |

`Timestamp`はUTCのRFC3339文字列（ミリ秒、末尾Z）へ保存する。期間は整数ミリ秒。文書上の時刻順で競合を解決せず、後述のrevisionとジョブIDを使う。

### 4.2 ProjectとArtwork

```text
Project { id, title, created_at, updated_at,
          artworks: Vec<Artwork>, references: Vec<Reference>, sessions: Vec<PracticeSession> }
Artwork { id, role: Copy|Memory,
          source_artwork_id: Option<ArtworkId>, status: Draft|Finalized,
          document: RasterDocument }
Reference { id, image: PixelGrid, source_title?, creator?, source_url?, usage_note? }
PracticeSession { id, exercise: ObservationMemory, status: InProgress|Completed|Abandoned,
                  stage, subject, reference_id, constraints, observations,
                  artifacts: Vec<StageArtifact>, reference_visibility,
                  elapsed_ms, events: Vec<PracticeEvent>, validation_note, reflection_draft,
                  reflection: Option<Reflection> }
```

MVPは1 Project＝1セッション、1つの埋込み参照、Copy／Memory各最大1成果物とする。`artifacts`が段階と作業対象の唯一の対応表で、`active_artwork_id`を重複保存しない。題材はセッションだけが持つ自由入力文字列とし、未定義の題材カタログIDを導入しない。Constraints.initial_paletteは課題開始時の色設定の記録で、Copyを作る初期値となる。作品内の編集済みパレットとは役割が異なり、課題開始後は変更しない。

パレットはArtworkごとの値として所有する。MemoryへはCopy確定時のパレット値だけを複製し、以降は独立する。RGBAモードなのでパレット編集は既存ピクセルを置換しない。ピクセル色の一括置換はMVP外とする。色数上限はセッションのConstraintsだけが所有し、実使用色数はalpha>0のRGBA集合から作品別に数える。半透明色も含め、透明は数えない。

Finalizedは画素だけでなくパレットを含むArtwork全体の不変性を意味する。`source_artwork_id`は同じProject内の由来だけであり、共有Celや差分保存にしない。CopyはNone、Memoryは対応CopyのIDとする。確定後の再編集・新しい作業版分岐はPhase 2へ延期し、MVPの「戻る」は閲覧に限定する。

FrameとLayerは画素の所在を指定する最小構造だけを残す。タグ、汎用ブレンドモード、共有Cel、Variation／Cleanup／Freeの未使用分岐はPhase 1では実装・保存しない。将来形式を変更してよく、互換性のために先行実装しない。

### 4.3 練習の原子的な状態遷移

`training::apply_session_command(&mut Project, command, context)`が検証・新規データ構築を終えてから一括適用する。失敗時はProjectもイベントも不変。UIへ「作成すべきArtwork」を返して部分適用させない。成功時の通知は表示更新と復旧保存要求だけで、ドメインの変更は完了済みとする。

| 現在段階 | コマンド／前提 | 適用後 |
| --- | --- | --- |
| Setup | StartObservation：題材、制約、参照を検証 | Observe、参照表示、観察タイマー開始 |
| Observe | CompleteObservation：観察3項目が非空 | Copy、透明なCopy Draftを作成 |
| Copy | CompleteCopy：対応Draft、編集ジェスチャーなし | Copyを確定し同時にMemory Draftを白紙生成、Memoryへ、参照Hidden |
| Memory | RevealReference：Hiddenのとき | Revealedと再表示イベント。重複要求は無変更 |
| Memory | HideReference：Revealedのとき | Hiddenへ。過去の再表示イベントは残る |
| Memory | CompleteMemory：対応Draft、編集ジェスチャーなし | Memory確定、Validateへ。reference_visibility=Revealed |
| Validate | CompleteValidation | Compareへ |
| Compare | CompleteComparison | Reflectへ |
| Reflect | CompleteReflection：3項目がtrim後非空、各140 Unicodeスカラー値以内 | reflectionを確定、Complete／Completedへ |
| 未完了の任意段階 | Abandon | Abandonedへ、再編集せず閲覧のみ |

段階遷移は表以外を拒否する。Setupでのみ題材・Constraints・参照を変更でき、Observe以降は課題設定を固定する。振り返り入力途中は`reflection_draft`、検証メモは`validation_note`として保存する。観察はObserve、検証メモはValidate、振り返り草稿はReflect、出典は未完了段階でのみSetコマンドを受け付け、入力確定値を復旧対象にする。Completed／Abandonedでは変更コマンドを拒否する。IME未確定文字列はUI状態である。タイマーはUIから単調時計で測った前景練習時間の差分を渡し、バックグラウンド・中断・アプリ停止中は加算しない。time_limitはセッション累計の目安、超過イベントは1回で編集を禁止しない。elapsed_msの反映は1秒単位と段階遷移・保存要求の直前とし、毎描画フレームを永続変更にしない。

`PracticeEvent`はStageCompleted、ReferenceRevealed、TimeLimitExceeded、SessionCompleted、SessionAbandonedの閉じた列挙とする。StageCompletedは完了した段階と累計経過時間を持ち、Copy／Memoryの場合だけArtworkId・実使用色数・alpha>0の画素数を追加する。参照イベントにはReferenceIdを持たせる。生入力・ツール操作の全記録、汎用payload、イベントソーシングは不要で、現在状態が再開の正本となる。

### 4.4 Copy→Memoryの白紙保証

Memory用の構築入口は`new_memory_blank(new_ids, size, palette)`とし、Copy文書や画素配列を引数に取らない。trainingがConstraintsから寸法を渡し、すべてのCelを`PixelGrid::transparent(size)`で新規確保する。Copy IDは構築後に由来として設定する。

段階確定と白紙作成、成果物リンク登録、参照非表示、イベント追記は同じProject変更になる。Memoryには別Artwork／Layer／Frame ID、新しいEditSession、空のUndo／Redoを与える。Copyの履歴、作業中Delta、選択、クリップボード、描画テクスチャを引き継がない。保存待ちでもこの原子性を崩さず、復旧ファイルには遷移前か遷移後のどちらかだけが入る。

Memory中は参照パネルだけでなくCopyの1×表示、比較、サムネイル、過去段階の閲覧経路を隠す。「参照を再表示」操作を経た場合だけこれらの閲覧を許可し、再度隠せる。再開時も保存されたHidden/Revealedを尊重する。これはアプリ内の学習導線の保証であり、利用者による外部画像閲覧の禁止ではない。

## 5. ピクセル編集エンジン

### 5.1 コマンドとトランザクション

可変文書をUIに公開しない。`EditSession`がDraftの作業文書を所有し、trainingは確定時に検証済み文書を受け取る。ProjectのアクティブDraftは最後の確定編集状態を保持し、ジェスチャー中の表示にはEditSessionの作業文書を使う。commit／Undo／Redoの候補をtrainingのDraft更新コマンドへ渡し、対象Draftと文書の検証成功後にProjectとEditSession履歴を同じUI処理内で確定する。拒否時は作業文書・履歴とも直前の確定状態へ戻し、片側だけを進めない。小さなMVPでは文書全体の値コピーを許容し、両方を独立に編集しない。

```text
EditCommand = PencilStroke | EraseStroke | Fill | SetPalette
EditDelta = Pixels { layer_id, frame_id, changes: Vec<PixelChange> }
          | Palette { before: Palette, after: Palette }
PixelChange { coordinate, before: Rgba8, after: Rgba8 }
```

コマンド列挙で十分であり、動的traitや汎用EditDocumentを別に作らない。対象ArtworkはEditSessionの所有者が保証し、pixel_coreからtrainingのArtworkIdへ依存させない。

- 1ジェスチャー＝1トランザクション。同一座標は最初のbeforeと最後のafterを保持し、最終的に同じなら除外する。commitで空なら履歴追加もRedo破棄もrevision更新もしない。
- commitが成功した新規変更だけがRedoを破棄する。Undoはbefore、Redoはafterを適用し、対象と現在値を検証してから全変更を一括適用する。失敗・キャンセルでは文書と履歴カーソルが元のままになる。
- 作業中に別の編集、Undo／Redo、ツール・作品切替、段階確定を実行しない。Escape／フォーカス喪失／モーダル開始はキャンセル。通常のポインター解放はcommit。明示保存と画面移動はジェスチャー完了まで保留する。
- パレット編集のダイアログはローカル草稿へ変更し、採用1回を1履歴とする。選択色は履歴対象外。削除／Undo後に無効な選択IDは先頭へ補正し、スポイトは未登録色をUIのRGBA選択として保持する。
- Undo範囲は現在Draftの画素とパレットのみ。セッション遷移、出典、振り返り等は専用コマンドで保存対象を更新し、画素Undoへ混ぜない。テキスト欄内のUndoはその入力欄の編集機能へ委譲する。
- 履歴は最大100確定トランザクション。上限では最古から破棄し、進行中履歴を破棄しない。確定Artworkは編集入口を拒否する。段階をまたぐUndo、再起動後のUndoは提供しない。

`ProjectRevision`はdesktopの実行時カウンターで、永続内容の実変更（commit、Undo、Redo、メタデータ、経過時間、段階遷移）ごとに増え、Undoでも減らない。値が保存済みと同じに戻っても未保存扱いでよい。画面専用操作では増やさない。描画用`render_epoch`はEditSessionの実行時値で、作業中変更・キャンセル・Undo／Redoでも増やす。これを保存revisionや履歴カーソルと兼用しない。

### 5.2 ツール

- Pencil/Eraser: ドラッグ点間を整数Bresenhamで補間し、高速移動でも穴を作らない。
- Fill: 選択レイヤー／フレームだけを4近傍で走査する。開始色と置換色が同じなら変更なし。
- Eyedropper: 選択レイヤーの生ピクセルを読み、透明なら選択色を変えない。
- Pan: 文書を変更しないUI操作であり、編集履歴に入れない。

入力座標は`screen -> canvas local -> integer pixel`の一方向変換に集約する。描画とヒットテストが同じ`CanvasTransform`を使い、ズーム倍率ごとのずれを防ぐ。

## 6. 描画と診断

描画パイプラインは次の順にする。

```text
PixelGridをレイヤー順に合成
→ 表示専用診断（シルエット／グレースケール／反転）
→ Nearest Neighborで拡大
→ グリッド、選択、警告マーカーを重ねる
```

1×プレビューと拡大キャンバスは同じ合成結果を共有する。テクスチャキャッシュのキーはOpenGeneration、ArtworkId、render_epoch、診断モードとする。表示設定変更時も再評価し、PNG出力にはキャッシュを使用しない。CompositeImageとDisplayImageはPixelGridと同じRGBA画像値を示す呼称であり、新しい画像階層は作らない。診断関数は`&PixelGrid -> PixelGrid`の純粋関数とし、編集データへの可変参照を受け取らない。

左右反転も表示座標だけを変換する。背景は合成結果の背後に描画し、PNG出力には含めない。

## 7. アプリケーション状態と画面遷移

```rust
struct AppState {
    route: Route,
    open_project: Option<LoadedProject>,
    editor: EditorUiState,
    save: SaveState,
}

enum Route {
    Home,
    ExerciseSetup,
    Workspace { session_id: SessionId },
    Compare { session_id: SessionId },
    Reflection { session_id: SessionId },
    History,
}
```

`EditorUiState`にはズーム、パン、選択ツール、選択色、診断表示、参照の表示変換、比較の点滅位相、ダイアログを置く。これらを`.dotted`の正規プロジェクトデータへ混ぜない。再開に必要な現在段階、作業中Artwork、参照の再表示履歴はドメイン側へ置く。

主要遷移は次のとおり。

```text
Home → Setup → Observe → Copy
                         ↓ 確定
                 参照を隠す＋白紙生成
                         ↓
                      Memory → Validate → Compare → Reflect → Home
```

段階とRouteを独立に進めない。Routeは保存された段階と許可された閲覧先から導出し、再開時に再構築する。MVPの過去段階は閲覧専用で、Memory中は4.4の表示制約に従う。

### 7.1 eguiの入力・描画契約

- `InputState.events`のPointerMoved／PointerButtonを到着順に処理し、1表示フレームに押下・移動・解放が揃っても1ストロークにする。末尾のpointer位置だけを読まない。Touchと合成Pointerイベントを二重処理しない。eguiの複数passでは同じ入力バッチを再実行しないよう、フレーム単位の処理済み印を持つ。[InputState](https://docs.rs/egui/latest/egui/struct.InputState.html)、[Event](https://docs.rs/egui/latest/egui/enum.Event.html)、[Context](https://docs.rs/egui/latest/egui/struct.Context.html)
- キャンバスに割り当てたRect内の一次ボタン押下からのみ描画を開始する。TextEdit／IME／モーダル／他widgetが使う入力を奪わず、画像Undoショートカットは編集キャンバスにフォーカスがある場合だけ処理する。OSのCommand修飾差を入力アダプターに閉じる。
- ジェスチャー中は対象とCanvasTransformを固定する。外へ出たらキャンバス境界まで整数線をクリップし、外から再入場した区間もクリップする。外での解放を受け取ればcommit、PointerGoneまたはフォーカス喪失で解放が確認できなければcancel。画面座標を端セルへclampして外側移動を描画しない。ズーム／パン変更はジェスチャー終了まで保留する。
- `screen`はeguiの論理point。`cell_points = integer_zoom / pixels_per_point`、セル座標は`floor((pos-origin)/cell_points)`。右端と下端は半開区間の外側。originは物理ピクセル境界にスナップする。1×は作品1画素＝画面1物理ピクセルとし、OSスケール変更時は進行中ジェスチャーをcancelしてからtransformを更新する。反転診断中の編集はMVPでは無効化し、入力の逆変換を不要にする。
- 入力適用→合成と診断→テクスチャ更新→拡大と1×の描画の順で両方へ同じ結果を渡す。テクスチャハンドルはUIで保持し、毎フレーム作り直さない。RGBAは`ColorImage::from_rgba_unmultiplied`へ渡しNearestサンプラーを指定する。GPU表示データを保存へ逆流させない。[ColorImage](https://docs.rs/egui/latest/egui/struct.ColorImage.html)、[TextureOptions](https://docs.rs/egui/latest/egui/struct.TextureOptions.html)
- 背景とグリッドは表示だけに重ねる。シルエットはalpha>0のRGBを単色にしてalphaを保持、グレースケールは線形sRGBへ変換後に輝度係数0.2126/0.7152/0.0722で計算しsRGBへ戻して丸め、alphaを保持する。これらは製品の表示規則として固定fixtureで検証する。
- ワーカーはCPUデータだけを返す。完了時は`request_repaint`、点滅比較（500 ms周期）とタイマー表示は`request_repaint_after`で次回更新を予約する。点滅／タイマーの経過はUI側の単調時計で測る。[Context](https://docs.rs/egui/latest/egui/struct.Context.html)

## 8. 保存設計

### 8.1 `.dotted` v1の契約

```text
manifest.json
artworks/{artwork_id}/{layer_id}/{frame_id}.png
references/{reference_id}.png
```

manifestは`format_version: 1`と4章のProjectを保存する。配列でArtwork／Layer／Frame／Palette／イベントの順序を表し、マップの走査順に依存しない。Cel項目はlayer_id、frame_id、path、sha256、参照項目はid、size、出典情報、path、sha256を持つ。SHA-256は格納されたPNGファイルの全バイトを対象にした小文字hexとする。画素はPNGのみを正としJSONへ重複保存しない。Artworkのサイズはdocument内の1箇所のみとする。

保存対象外はUndo／Redo、render_epoch、ProjectRevision、選択色、ズーム、パン、参照の表示変換、テクスチャ、保存パス、ジョブ状態である。参照のHidden/Revealedは練習状態なので保存する。復旧と通常保存でProjectのスキーマは同一とする。復旧ファイル名を`{open_generation}/{revision}.dotted`とし、復旧候補の識別はパス、Project情報、ファイル時刻から得る。追加sidecarを正本にしない。

v1は1セッション・参照1枚・Artwork最大2個、各Artworkは16×16か32×32でConstraintsと同寸、1レイヤー・1フレーム・1Celとする。Setup／Observeでは成果物0個、CopyではCopy Draftのみ、MemoryではCopy Finalized＋Memory Draft、Validate以降は両方Finalized。CompletedはCompleteかつ有効なreflection必須、Abandonedは中断段階の構造を保持する。空白Memoryの保証は作成時に検証し、編集済みMemoryを読込時に消去しない。

読込は一時的なDTOへ行い、次をすべて検証してから開いているProjectを置換する。

- format_versionは完全一致。不明フィールド、重複JSONキー、未知enum、不足フィールドは拒否し、未対応データを黙って落とさない。
- エントリーは上記の正規パスのみ、UUIDは小文字ハイフン表記。絶対パス、`..`、バックスラッシュ、重複名、シンボリックリンク、暗号化、余分なファイルは禁止。ZIPはディスクへ展開せず上限付きストリームで読む。
- 入力ZIP 80 MiB、manifest 1 MiB、エントリー数4、展開後合計80 MiB、参照各辺4096以下・総画素16,777,216以下を上限とする。ヘッダー申告だけでなく実際の読み取り量とデコード時のメモリ上限を検証し、掛け算はchecked演算とする。
- 全IDの所有スコープ内一意性（PaletteEntryIdは各Palette内、LayerId／FrameIdはProject内）、成果物リンクの段階／role／status、参照先存在、Copy→Memoryの由来、Constraints、パレット、テキスト制約、時刻、イベント通し番号と段階の整合を検証する。無関係な成果物と二重リンクは拒否する。
- 各Celと参照のPNGパスがID由来の期待パスと一致し、PNGは静止RGBA8、期待寸法、ハッシュ一致、alpha=0のRGB=0であること。内部PNGの形式違いや隠れRGBは破損扱いとし、読込時に修復しない。

これらの上限はMVPの実装値として保存前にも検証する。イベント最大256件、出典等の通常文字列は各4096 Unicodeスカラー値以内（振り返りは140）、ドメイン構築・変更時にも上限を検証し、超過時は状態不変のエラーを表示する。観察・検証メモは各4096、reflection_draftも各140とする。形式の拡張時には上限とversionを一緒に見直す。

### 8.2 ファイル置換と復旧

同じディレクトリに一意の一時ファイルを作成→ZIPをfinalize→バッファflush→ファイル`sync_all`→原子的置換の順に行う。旧ファイルの事前削除はしない。対応OS／ファイルシステムで置換失敗時に旧ファイルが残ることを試験する。renameだけで全OSの停電耐性を保証しない。親ディレクトリ同期が可能な環境では実施し、置換後の同期失敗は「置換済み・耐久性未確認」と返して置換前の失敗と区別する。[Rust rename](https://doc.rust-lang.org/std/fs/fn.rename.html)、[File::sync_all](https://doc.rust-lang.org/std/fs/struct.File.html#method.sync_all)

プロセスクラッシュからの復旧点は**最後に書込み完了が確認できたスナップショット**とする。編集確定とディスク保存完了は別であり、直前の未保存操作の復旧を約束しない。UIは「未保存／保存中／復旧保存済み（時刻）／保存失敗」と明示保存先への未保存状態を分けて示す。復旧保存に成功しても明示保存済みとは表示しない。

復旧ファイルはアプリ管理領域のOpenGenerationごとの場所へ保存し、ProjectIdが同じ別プロセスの復旧ファイルと衝突させない。再起動時は有効な候補を時刻・題材・段階で提示し、ユーザーが開く。破損候補は明示し、通常保存へ無断で上書きしない。同じ世代では新しい復旧保存成功後に古い世代内snapshotを削除し、有効な最新版を常に残す。未解決の復旧データは期限で自動削除せず、同じ内容以上の明示保存成功または利用者の破棄で削除する。保存rの成功後もr+1の復旧を削除しない。

### 8.3 自動保存と非同期処理の競合規則

`SaveState`は`current_revision`、`explicit_saved_revision`、`recovery_saved_revision`、保存先、進行中ジョブ、保留要求を持つ。ワーカー入力はUIが確定境界で採ったProject全体の独立snapshotと`{job_id, open_generation, project_id, revision, kind, destination}`。ライブProjectやEditSessionへの参照を渡さない。

- 編集・メタデータ確定後1秒のアイドル、または未保存が5秒続いた場合に復旧要求を出す。段階遷移・振り返り確定時は即時要求する。ドラッグ中は最後の確定Projectだけを保存する。
- MVPは単一I/Oワーカーで保存・読込・PNG出力・SQLiteを直列実行する。同じ保存先への書込み完了順が要求順を追い越すことはない。復旧要求だけは未着手分を最新snapshot1件にまとめる。明示保存・別名保存・出力の要求は黙って捨てない。
- 保存r完了時、同じOpenGenerationと対象先のジョブだけが該当saved_revisionをrへ進める。currentがr+1なら未保存のまま次を要求する。Undoで内容が戻ってもrevisionは巻き戻らない。失敗ではsaved_revisionを進めない。
- 別名保存中は保存先変更操作を無効化し、成功後だけ新しい明示保存先を採用する。編集中に成功した古いsnapshotを現在保存済みと扱わない。復旧と明示保存のdestinationが同じになる指定は拒否する。
- Open／New／Closeの前に進行中ジェスチャーを解決し、未保存内容を保存・復旧保持・破棄のどれにするか確定する。保存または復旧保持を選んだ場合は必要な書込み成功まで閉じない。読込待ち中は編集を無効化し、全検証成功時だけProjectを置換する。Open要求に新しいgenerationを割り当て、現在LoadedProjectの世代は読込成功まで保持する。失敗／取消時は元Projectを編集可能に戻す。新しいProjectを採用後は旧世代の読込・保存通知を現在画面へ適用しない。参照デコードは同じ世代でも最新job_idとSetup段階が一致する場合だけ採用する。
- 終了時も保存完了を待つか明示的な破棄を選ぶ。保存失敗・ワーカー停止は未保存表示と再試行を残す。ジョブ取消で既に書込み済みのファイルが元に戻るとは扱わない。
- 同じ明示ファイルを複数アプリから編集する運用はMVP非対応。保存先単位の協調ロックを取り、競合時は別名保存を提示する。外部変更は読込時／保存成功時のファイル指紋との差で検出し、競合時に無断上書きしない。非協調プロセスとの完全な同時更新保証はしない。

SQLiteは保存成功後にのみ索引を更新し、索引失敗で.dotted保存成功を取り消さない。主キーは保存場所（同じProjectIdの別名保存を許可）、列はProjectId・表示名・更新日時・題材・サイズ・色上限・段階・完了状態。復旧ファイルを通常履歴へ混ぜない。管理ディレクトリと利用者が指定するフォルダー内の.dotted走査で再構築できる。任意の外部保存先は再指定が必要で、ファイルパスの消失時もピクセルはSQLiteから復元しない。

### 8.4 PNGと参照の入出力

作品出力は現在の確定境界のArtworkを対象とし、原寸／2／4／8倍の整数複製でRGBA8 PNGへ書く。MVPは単一レイヤーなので元Celと完全一致し、半透明を保存する。診断、グリッド、参照、背景を含めない。出力失敗はプロジェクト保存状態へ影響しない。

参照インポートはMVPでは静止PNG／JPEGに限定し、入力64 MiB、寸法・画素上限は8.1と共通、デコーダー作業メモリは256 MiB以内に制限する。向き情報を適用してRGBA8へ変換し、alpha=0を正規化してPNGへ埋め込む。APNG等の動画と16bit画像は説明付きで拒否する。勝手なリサイズ・減色はしない。外部参照の正規化後の画素を往復一致の基準とし、不要な元メタデータは保持しない。出典は明示フィールドに記録する。作品PNGを編集キャンバスへ取り込む機能はMVP外で、再読込一致はデコーダーを使う自動テストで確認する。

## 9. エラー処理

エラーは利用者が次の行動を選べる粒度にする。

- `ValidationError`: 不正なサイズ、必須入力不足など。該当入力の近くへ表示する。
- `EditError`: 範囲外座標、不整合な対象IDなど。通常はUIの不具合としてログし、安全に操作を無効化する。
- `ProjectIoError`: 読込不能、破損、保存権限不足、容量不足。既存データを保持し再試行または別名保存を提示する。
- `ExportError`: 書出しだけの失敗。プロジェクト編集状態には影響させない。

パニックは回復不能な内部不変条件違反に限定し、ユーザー入力や壊れたファイルで発生させない。ログに参照画像、作品ピクセル、自由記述を出さない。

## 10. テスト設計

### 10.1 コアfixture

8×8以下のASCII表現と明示RGBA配列を併用し、期待画素をレビュー可能にする。コアの小寸法fixtureと.dottedの16／32サイズ制約を混同しない。

### 10.2 ドメイン検証

trainingでは状態遷移と不変性をメモリ内で検証する。保存後の再開はproject_io／desktopの統合テストに置き、trainingのテストからproject_ioへ逆依存しない。

### 10.3 永続化fixture

各保存段階の正常fixtureと、項目を1つずつ壊した異常fixtureを使う。時計と書込みの失敗箇所を制御し、実際のファイル置換は両OSでも確認する。

### 10.4 UI検証

入力バッチ処理をegui表示から分けて自動テスト可能にする。物理等倍、Nearest表示、IME、OS境界の入力は実機QAで補完する。ProjectRevisionの不変性もdesktopで検証する。

### 10.5 受け入れ条件とテストの対応

テスト名は実装時に作成する契約IDであり、現時点で実行済みを意味しない。自動テストは固定入力・期待値で合否を判定し、UI手動QAはOS・DPI・倍率・操作・結果を記録する。

| 仕様ID | 所有者／テストID | 必須シナリオと期待結果 |
| --- | --- | --- |
| AC-01 | training `setup_constraints`、desktop `first_run` | 16／32、色上限2／8の境界、無効設定拒否。同梱参照・プリセットで初回3操作以内、参照ファイル選択不要 |
| AC-02 | training `memory_atomic_blank`、desktop `memory_visibility` | Copyに全セル異なるRGBAを入れて遷移し全Cel=0、ID分離、パレット独立、Undo空。Memory編集後もCopy不変。誤ID・重複遷移は全状態不変。Copyプレビュー／比較／履歴／再開からの漏れなし、Revealだけが記録付きで表示 |
| AC-03 | project_io `project_roundtrip`、desktop `resume_stage` | SetupからCompleteまで各段階の有効fixture、Hidden／Revealed、半透明、出典、草稿、イベント、時刻、順序の構造的等価。正規化済み画素の全バイト一致、外部参照元がなくても再開 |
| AC-04 | desktop `preview_frame`／実機QA | 1／2／4／8／16／32倍、DPI 1／1.25／1.5／2、両OSで作業中とcancel直後の両ビュー一致。スクリーンショットで1×物理寸法を確認、代表32×32 fixtureで入力→更新16 ms目標を計測 |
| AC-05 | pixel_core `stroke_clip`、desktop `input_batch` | 単点・四隅・右下半開境界、同フレーム押下→複数移動→解放、外へ移動→再入場／外で解放、負座標、PointerGone、フォーカス喪失、DPI変更、モーダル・TextEdit・IMEとの競合、複数passで重複commitなし |
| AC-06 | pixel_core `tools_palette_colors` | Bresenham全方向、Fillの4近傍／同色無変更、透明スポイト無変更、未登録色取得、半透明RGBA別色、透明除外、パレット採用／削除で元画素不変。上限超過の警告と継続描画をUI確認 |
| AC-07 | pixel_core `display_readonly`、desktop `compare_modes` | 診断前後の元文書完全一致、シルエットalpha保持、グレー固定期待値、反転中編集不可、背景／グリッド切替でdirty不変。横並びと500 ms点滅が静止中も更新し非破壊 |
| AC-08 | pixel_core `edit_history_invariants`、training `finalized_guard` | A→B→Cを1 UndoでA、A→B→Aは空。cancel／no-op後Redo維持、新規commit後のみ破棄、Undo／Redo逆操作一致、失敗途中適用なし、パレットUndo、100件境界、確定版編集拒否、段階越え不可 |
| AC-09 | project_io `png_exact_scale` | alpha 0／1／127／254／255、全チャンネル境界値のfixture。各元セルがn×nへ複製され、背景・参照・診断なし。出力失敗でProject状態不変 |
| AC-10 | desktop `save_scheduler`、project_io `recovery_crash` | 制御可能なワーカーと時計でr保存中→r+1編集／Undo、復旧と明示保存の交錯、要求集約、別名保存成功／失敗、Open A→BでAの遅延応答、終了待ち、ワーカー停止、同一先競合を検証。書込み前／途中／置換前／置換後でプロセスを停止し有効な旧版か新版へ復旧 |
| AC-11 | project_io `invalid_container`／`atomic_replace` | 欠落／重複／未知項目・path traversal・symlink・暗号化・サイズ申告偽装・算術overflow・PNG寸法／hash／alpha不整合・IDと段階不整合を拒否。容量不足・権限・rename・sync失敗を注入、旧版保持と置換後耐久性未確認を区別。両OSで実ファイル試験 |
| AC-12 | training `transition_table`／`reflection_timer`、project_io `history_rebuild`、desktop `practice_loop` | 全Stage×全Commandの許否・状態・イベント数を表と照合。空白／140／141文字、IME草稿、時刻逆行でも経過時間非負、超過1イベント、参照再表示。索引失敗後も作品保存成功、索引削除→指定フォルダーから再構築、欠損ファイル表示、履歴から同段階再開 |

依存検査`workspace_dependencies`はworkspace内部について3章の辺以外を拒否する。共有fixtureのための循環dev依存も作らない。自動検証はCargo workspace作成後に`cargo fmt --check`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`と依存検査をCIへ登録する。

ユーザビリティはPhase 1で初心者5～10人に説明なしの30分セッションを実施し、開始操作数、完了有無、模写と記憶描きの理解、1×での判断、具体的な次回改善点を記録する。能力スコアにせず、未完了の理由を改善へ使う。機能テストの成功だけで学習仮説を検証済みとしない。

## 11. 実装順序

各縦切りで動く画面と自動テストを残す。

1. **基盤**：Cargo workspace、ID、RGBA、PixelGrid、固定fixture、CI。
2. **最小編集**：Pencil、Eraser、座標変換、Nearest Neighbor表示、1×プレビュー、Undo/Redo。
3. **色編集**：Palette、Fill、Eyedropper、使用色数と上限警告。
4. **練習縦切り**：Setup、Observe、Copy、白紙Memory、段階遷移。
5. **永続化**：`.dotted`往復、復旧自動保存、SQLite履歴索引。
6. **検証と比較**：シルエット、グレースケール、反転、背景、横並び、点滅。
7. **完了ループ**：振り返り、履歴、PNG出力、未完了再開。
8. **MVP仕上げ**：アクセシビリティ、ショートカット、性能、破損入力、初心者テスト。

機能実装後には変更領域全体を見直し、重複、責務漏れ、不要な抽象化、命名、古いコメントを整理してから再検証する。

## 12. 性能目標

- Pencil操作から拡大表示と1×プレビュー更新まで16 ms以内を目安とする。
- 32×32のMVP編集では全グリッド再合成を許容し、測定なしに差分描画を複雑化しない。
- 保存と参照デコードはUIスレッドを塞がない。
- 入力イベントが表示フレームより多い場合も、座標列を捨てずに1トランザクションへまとめる。

Phase 3で大きなフレーム数が必要になった時点で、dirty領域、テクスチャ差分更新、共有Cel等を計測に基づいて検討する。

## 13. MVP開始前に固定する詳細

以下は基盤スパイクで検証し、機能実装を進める前に本書へ確定値を記録する。

- 対応OSの最低バージョンとキーボードショートカット差
- OSごとのアプリ管理領域の具体パス（保持方針は8.2に従う）
- egui／画像／ZIP／時刻ライブラリのバージョン固定（保存表現は4.1に従う）
- eguiでのテクスチャ更新方法を検証する小さなスパイク結果

OSごとの保存置換、DPI・複数pass・IMEの挙動は文書確認だけでは実機保証にならない。スパイクの検証結果を実装開始条件とし、依存更新時に該当QAを再実行する。

## 14. Phase 2以降への拡張点

- サイズ再解釈はArtwork数・role・Constraints・由来規則と形式versionを拡張し、独立した複数Artworkを使う。
- 高度診断は読み取り専用関数として追加し、編集コマンドにしない。
- 統計、提案、実績は保存済み`PracticeEvent`から導出し、手書きの集計値を正本にしない。
- アニメーションはFrame／Celを拡張し、Tagと保存形式versionを必要時に追加する。
- タイル表示とゲーム背景は表示アダプターとして追加し、PixelGridを変形しない。

ただし将来拡張のためだけのプラグイン機構、汎用イベントバス、ネットワーク同期抽象化はMVPに入れない。現在必要な境界を小さく保ち、実際のPhase 2要求に合わせて拡張する。
