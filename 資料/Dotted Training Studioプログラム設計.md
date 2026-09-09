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
| 非同期処理 | 保存、読込、画像デコードだけワーカースレッド | 描画コマンドの順序と決定性を守る |

ファイル拡張子は仮に`.dotted`とする。形式には`format_version`を持たせるが、開発中の旧形式を維持する互換層は作らない。

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
  test_support/
```

依存方向は`desktop -> training -> pixel_core`、`desktop -> project_io -> {training, pixel_core}`に固定する。`pixel_core`と`training`からUI、ファイルダイアログ、OS API、SQLiteへ依存してはならない。

## 4. ドメインモデル

### 4.1 識別子と基本型

IDは型を分け、取り違えをコンパイル時に防ぐ。

```rust
struct ProjectId(Uuid);
struct SessionId(Uuid);
struct ArtworkId(Uuid);
struct LayerId(Uuid);
struct FrameId(Uuid);
struct ReferenceId(Uuid);

struct CanvasSize {
    width: NonZeroU16,
    height: NonZeroU16,
}

#[derive(Clone, Copy, Eq, PartialEq, Hash)]
struct Rgba8 { r: u8, g: u8, b: u8, a: u8 }
```

MVPの新規作成UIは16×16と32×32に限定する。コアは将来の任意サイズに備えるが、メモリ上限と画像デコード攻撃を避けるため最大寸法と総ピクセル数を検証する。

### 4.2 ProjectとArtwork

```rust
struct Project {
    id: ProjectId,
    title: String,
    created_at: Timestamp,
    updated_at: Timestamp,
    palette: Palette,
    artworks: OrderedMap<ArtworkId, Artwork>,
    references: OrderedMap<ReferenceId, ReferenceMeta>,
    sessions: OrderedMap<SessionId, PracticeSession>,
}

struct Artwork {
    id: ArtworkId,
    subject_id: String,
    role: ArtworkRole,
    source_artwork_id: Option<ArtworkId>,
    size: CanvasSize,
    layers: Vec<Layer>,
    frames: Vec<Frame>,
    tags: Vec<FrameTag>,
    revision: u64,
}

enum ArtworkRole { Copy, Memory, Variation, Cleanup, Free }
```

`source_artwork_id`は由来だけを表し、データ共有や差分保存には使わない。段階を確定するとArtworkを不変スナップショットにし、再編集は新しい作業版から行う。

MVPのUIは各Artworkにつき1フレーム、1枚の表示可能な`Sprite`レイヤーだけを生成する。モデル上のフレームとレイヤーの軸はPhase 3で保存形式を壊さず拡張するため残す。

### 4.3 ピクセル格納

```rust
struct Layer {
    id: LayerId,
    name: String,
    role: LayerRole,
    visible: bool,
    opacity: u8,
    cels: OrderedMap<FrameId, PixelGrid>,
}

struct PixelGrid {
    size: CanvasSize,
    pixels: Vec<Rgba8>, // row-major、長さはwidth * height
}
```

透明ピクセルは`Rgba8(0, 0, 0, 0)`へ正規化する。完全透明色の隠れたRGBを保持しないことで、同じ見た目なのに色数や保存結果が異なる状態を防ぐ。座標変換は必ず境界検査付きAPIを通し、UIが`pixels`を直接変更しない。

### 4.4 パレット

```rust
struct Palette {
    entries: Vec<PaletteEntry>,
    color_limit: NonZeroU8,
    selected: PaletteEntryId,
}
```

透明は描画色ではなく消去結果として扱い、色数上限から除外する。使用色数はパレット登録数ではなく、比較対象Artworkの不透明ピクセルに実在するRGBA値の集合から算出する。上限超過は`Diagnostic`を返すだけで編集を拒否しない。

### 4.5 練習セッション

```rust
enum Stage { Setup, Observe, Copy, Memory, Validate, Compare, Reflect, Complete }
enum SessionStatus { InProgress, Completed, Abandoned }

struct PracticeSession {
    id: SessionId,
    exercise: ExerciseType,
    status: SessionStatus,
    stage: Stage,
    constraints: Constraints,
    observations: ObservationNotes,
    artifacts: Vec<StageArtifact>,
    events: Vec<PracticeEvent>,
    reflection: Option<Reflection>,
}
```

段階遷移は`PracticeSession::transition(command)`だけが行う。遷移結果は、作成すべき白紙Artwork、確定すべきスナップショット、参照表示状態などを`SessionEffect`として返す。

特に`Copy -> Memory`では、新しい`PixelGrid::transparent(size)`を生成する。copyのピクセル配列を入力として受け取らないAPIにし、「白紙から開始」を構造的に保証する。

```rust
enum SessionCommand {
    CompleteObservation,
    CompleteCopy { artwork: ArtworkId },
    StartMemory,
    RevealReference,
    CompleteMemory { artwork: ArtworkId },
    CompleteValidation,
    CompleteComparison,
    CompleteReflection(Reflection),
}
```

`RevealReference`はMemory段階だけで記録され、失敗にはしない。タイマーはOS時刻をドメインへ直接読ませず、UIから経過時間を渡す。制限超過も編集禁止ではなくイベントと表示状態にする。

## 5. ピクセル編集エンジン

### 5.1 コマンドとトランザクション

すべての変更は次の入口を通す。

```rust
trait EditCommand {
    fn apply(&self, document: &mut EditDocument) -> Result<EditDelta, EditError>;
}

struct EditDelta {
    artwork_id: ArtworkId,
    frame_id: FrameId,
    layer_id: LayerId,
    changes: Vec<PixelChange>,
}
```

`PixelChange`は座標、変更前、変更後を保持する。Undoは逆順に変更前を、Redoは変更後を適用する。同一値への書込みは履歴へ含めない。

ポインター押下でトランザクションを開始し、移動中は1つの作業中Deltaへ座標ごとの差分を統合し、解放時に1履歴として確定する。キャンセル時は作業中Deltaを巻き戻す。塗りつぶしとパレット変更はそれぞれ1コマンドである。新しい編集確定後はRedoスタックを破棄する。

履歴はセッションの自動保存対象に含めず、クラッシュ復旧は最後に確定した現在ピクセルを復元する。通常の明示保存後にUndoできるかはメモリ内履歴で判定し、再起動後のUndoまではMVP対象外とする。

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

1×プレビューと拡大キャンバスは同じ合成結果を共有する。ピクセル変更時にArtworkの`revision`を増やし、テクスチャキャッシュはrevision不一致時だけ更新する。診断関数は`&CompositeImage -> DisplayImage`の純粋関数とし、編集データへの可変参照を受け取らない。

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

`EditorUiState`にはズーム、パン、選択ツール、診断表示、ダイアログだけを置く。これらを`.dotted`の正規プロジェクトデータへ混ぜない。再開に必要な現在段階、作業中Artwork、参照の再表示履歴はドメイン側へ置く。

主要遷移は次のとおり。

```text
Home → Setup → Observe → Copy
                         ↓ 確定
                 参照を隠す＋白紙生成
                         ↓
                      Memory → Validate → Compare → Reflect → Home
```

段階を戻る場合も確定済みArtworkは変更しない。過去段階を編集する操作は、その段階を元に新しい作業版を作る明示操作として扱う。

## 8. 保存設計

### 8.1 `.dotted`コンテナ

```text
manifest.json
artworks/{artwork_id}/{layer_id}/{frame_id}.png
references/{reference_id}.png
```

`manifest.json`にはメタデータ、パレット、フレーム順、レイヤー順、セッション、イベント、振り返り、各PNGのパスとSHA-256を保存する。PNGはRGBA8、原寸、補間なしとする。

読込時は次を検証する。

- ZIPパスが相対パスであり、`..`を含まないこと
- 展開後サイズ、画像寸法、ピクセル総数が上限内であること
- 必須IDの一意性と参照整合性
- PNG寸法とmanifestのCanvasSizeの一致
- ハッシュ一致

保存は同一ディレクトリの一時ファイルへ完全に書き、flush後にrenameで置換する。処理失敗時は既存ファイルを保持する。

### 8.2 自動保存と履歴

- 編集トランザクション確定後にdirtyとし、短いアイドル後に復旧スナップショットを書く。
- 段階遷移と振り返り確定時は即時に復旧保存を要求する。
- 保存中に追加編集された場合はrevisionを比較し、完了後もdirtyを維持して再保存する。
- SQLiteはプロジェクトID、表示名、ファイルパス、更新日時、セッション概要だけを索引する。正本は`.dotted`であり、索引は再構築可能にする。
- SQLiteへ任意の作品ピクセルや参照画像を重複保存しない。

### 8.3 PNG入出力

PNG出力は対象Artworkをレイヤー合成し、原寸または2/4/8倍へ最近傍で複製する。診断、グリッド、参照、背景を含めない。再読込テストではデコード後RGBA配列の完全一致を確認する。

参照画像は一般画像をデコード後、表示用RGBA8 PNGへ正規化して埋め込む。EXIF等の不要なメタデータは保持せず、出典情報はmanifestの明示フィールドに保存する。

## 9. エラー処理

エラーは利用者が次の行動を選べる粒度にする。

- `ValidationError`: 不正なサイズ、必須入力不足など。該当入力の近くへ表示する。
- `EditError`: 範囲外座標、不整合な対象IDなど。通常はUIの不具合としてログし、安全に操作を無効化する。
- `ProjectIoError`: 読込不能、破損、保存権限不足、容量不足。既存データを保持し再試行または別名保存を提示する。
- `ExportError`: 書出しだけの失敗。プロジェクト編集状態には影響させない。

パニックは回復不能な内部不変条件違反に限定し、ユーザー入力や壊れたファイルで発生させない。ログに参照画像、作品ピクセル、自由記述を出さない。

## 10. テスト設計

### 10.1 `pixel_core`

- Pencilの単点、水平、垂直、斜線、高速ドラッグ補間、重複点
- 四隅と範囲外クリップ
- Eraserの透明色正規化
- Fillの4近傍、境界、透明領域、同色置換
- Eyedropperが合成色でなく選択レイヤー色を返すこと
- 1ドラッグ＝1 Undo、Redo破棄、キャンセル
- 色数が透明を除き、未登録でも実使用RGBAを数えること
- 診断後に元配列とrevisionが不変であること

固定fixtureは8×8以下のASCII表現からPixelGridを作り、期待配列をレビューしやすくする。

### 10.2 `training`

- 許可／不許可の全段階遷移
- Copy確定後のMemoryが透明な別配列・別Artwork IDであること
- 参照再表示がイベントになるが進行を妨げないこと
- 必須3項目、各140文字の振り返り検証
- 中断状態を保存・読込後に同じ段階から再開できること

### 10.3 `project_io`

- Projectの保存→読込の構造的等価性
- 全RGBA、透明度、順序、ID、時刻、イベントの往復
- 原寸／整数倍PNGの期待ピクセル
- 一時ファイル書込み失敗時に旧ファイルが残ること
- 不正ZIPパス、巨大寸法、不正参照、ハッシュ不一致の拒否

### 10.4 UIと手動QA

- 16×16と32×32の各倍率で四隅をクリックし、同じセルが変わる
- 描画の同一フレーム内に1×プレビューが更新される
- Homeから推奨練習を3操作以内に開始できる
- 参照非表示後にMemoryが白紙である
- 比較の横並び／点滅が元データを変えない
- クラッシュ相当の強制終了後に最後の確定操作まで戻る

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

以下は実装初日に短いArchitecture Decision Recordとして確定する。

- 対応OSの最低バージョンとキーボードショートカット差
- `.dotted`の最大展開サイズ、最大寸法、最大Artwork数
- アプリ管理領域と復旧スナップショットの保持期間
- JSON日時表現と時刻ライブラリ
- eguiでのテクスチャ更新方法を検証する小さなスパイク結果

これらは製品の練習体験を変える未決定事項ではなく、安全な実装値である。プロトタイプ計測後に確定してよい。

## 14. Phase 2以降への拡張点

- サイズ再解釈は複数Artworkと`source_artwork_id`をそのまま利用する。
- 高度診断は読み取り専用の`DiagnosticProvider`として追加し、編集コマンドにしない。
- 統計、提案、実績は保存済み`PracticeEvent`から導出し、手書きの集計値を正本にしない。
- アニメーションは既存のFrame、Cel、TagをUIへ開放し、保存形式の根本変更を避ける。
- タイル表示とゲーム背景は表示アダプターとして追加し、PixelGridを変形しない。

ただし将来拡張のためだけのプラグイン機構、汎用イベントバス、ネットワーク同期抽象化はMVPに入れない。現在必要な境界を小さく保ち、実際のPhase 2要求に合わせて拡張する。
