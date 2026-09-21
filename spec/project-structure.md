# Foundry プロジェクト構成

状態: MVP実装中。2026-09-21更新。

## 1. 目的と境界

Foundryは、画像変換や文書処理に必要な実装・依存環境をユーザー環境へ集約し、複数プロジェクトからCLIとして再利用する基盤である。

最初の利用場面はJPG / PNGのWebP変換とスクリーンショットの自動軽量化とする。次に外部変換ツールとOfficeパッケージ操作へ広げる。利用プロジェクトへのライブラリ、SDK、Python環境の追加を要求しない。

初期対象OSはmacOSとする。CLIと処理本体は可能な範囲でOS依存を避け、Windows / Linuxの配布・自動化対応は後続段階で判断する。

## 2. 構成予定

以下は最終的な初期構成の見取り図であり、空のcrateや将来機能のディレクトリを先に大量作成しない。

```text
foundry/
├── README.md
├── Cargo.toml                 # workspaceと共通依存
├── Cargo.lock                 # CLIの依存解決結果を管理
├── rust-toolchain.toml        # 開発用stableツールチェーン
├── crates/
│   ├── foundry-cli/           # 公開コマンド、引数、結果出力
│   │   └── tests/             # CLI integration tests
│   ├── foundry-core/          # 結果型、エラー、能力定義など
│   ├── foundry-image/         # 画像処理と画像worker
│   └── foundry-office/        # Office処理とOffice worker
├── adapters/
│   └── markitdown/            # 外部CLIを呼ぶRust crate
├── tests/
│   └── fixtures/             # 自作または利用条件が明確な検証素材
├── scripts/                  # ビルド・配布・検証の補助
├── integrations/
│   └── macos/                # OS側の自動変換設定と解除手順
├── spec/
│   ├── project-structure.md
│   └── implementation-plan.md
└── .github/workflows/        # 基本CI
```

公開UXは`foundry`一つとする。現在のMVPでは`foundry-image`をライブラリcrateとしてCLIへリンクしている。画像処理の利用実績と配布サイズを計測したうえで、重いNative依存を専用workerへ分離する。Office追加時のworker分離も同時に判断する。workerを導入した場合も内部実装とし、直接利用する公開APIとして扱わない。

crateを分割するだけでは実行バイナリの依存は分離されない。公開CLIが画像・Office処理ライブラリを直接リンクしない構成にする。動的Plugin探索や独自RPC基盤は作らず、静的な対応表と小さな引数・JSON契約でdispatchする。

## 3. 責務と依存方向

| 部分 | 責務 | 持ち込まないもの |
| --- | --- | --- |
| CLI | 引数解析、dispatch、公開結果の整形 | 画像codec、OOXML固有処理 |
| Core | 共通結果型、エラー識別子、能力定義 | 巨大な汎用framework |
| Native worker | 各形式の検証と処理 | 他のCapability固有ロジック |
| Adapter | 外部コマンドの検出、実行、出力の変換 | 外部ツールのソースコピー |
| OS integration | 保存検知、完了待ち、重複抑止 | 画像変換の再実装 |

CLI、worker、AdapterからCoreを利用する。Coreから個別機能には依存しない。現在の画像機能はCLIからライブラリcrateを呼ぶ。共有処理は複数箇所で必要になってから抽出する。外部プロセスはshell文字列を組み立てず引数配列で起動し、タイムアウトと終了状態を扱う。

## 4. Rust資材の統一

| 資材 | 方針 |
| --- | --- |
| 画像・Office処理のRust実装 | Foundry workspaceへ集約 |
| crate依存 | `[workspace.dependencies]`で共通宣言し、各crateで必要なものだけ利用 |
| 依存解決 | リポジトリの`Cargo.lock`を管理し、CI・配布ビルドでは`--locked`を利用 |
| ツールチェーン | リポジトリの`foundry-rust.toml`と`rust-toolchain.toml`で要求を固定し、実体はrustupでユーザー単位に共有 |
| ビルド成果物 | project・checkout・build unit・host tripleごとの外部targetへ隔離し、最終成果物は各プロジェクトが所有 |
| インストール済みworker | CLIと同一リリース単位で管理 |
| 他プロジェクトのRust依存や`target` | Cargo source cacheは共有するが、target内容は一律に統合しない |
| 開発環境全体のCargoキャッシュ | `~/.cargo`のdownload cacheをCargoの排他制御で共有し、compile結果はsccacheで内容アドレス共有 |

「一度構築する」はプロジェクトごとの再構築を避ける意味であり、更新やOS・CPUの違いによる再ビルドまで不要になるという意味ではない。

## 5. 配布と実行環境

開発時のworkspaceとインストール時の配置を分ける。配置候補は公開CLIを`~/.local/bin/foundry`、workerを`~/.local/lib/foundry/<version>/`とする。正式な配置はmacOSの配布方法を調査して決める。

内部workerは管理された配置先から解決し、任意のPATH上の同名ファイルへ無条件に委譲しない。CLIとworkerのバージョン不一致は診断する。更新は一つのリリース単位で切り替え、直前のリリースへ戻せる配置を設計する。

外部ツールは明示設定またはPATHから検出し、実体パスとバージョンを確認可能にする。独自dependency resolverは作らない。通常実行ではインストール、ネットワーク利用、外部サービス利用を暗黙に開始しない。

## 6. CLIと結果契約

### 共通規約

- 非対話実行を基本とし、操作途中の確認待ちを作らない。
- 終了コードは`0`が成功、`1`が処理失敗、`2`が引数・使用法エラー。
- 通常モードではstdoutに結果、stderrに警告・診断を出す。
- `--json`では、捕捉可能な成功・失敗ともstdoutに単一のJSONを出す。外部ツールの生出力を混ぜない。補足診断はstderrへ出す。
- help / versionはメタコマンドとし、初期段階でJSON対応を要求しない。
- CLIで扱える引数エラーもJSONモードの契約に含める。プロセス強制終了などはJSON保証の対象外。

共通エンベロープの予定形:

```json
{
  "schemaVersion": 1,
  "command": "image.webp",
  "ok": true,
  "data": {
    "input": "/path/input.png",
    "output": "/path/input.webp",
    "inputBytes": 100000,
    "outputBytes": 30000
  },
  "warnings": [],
  "error": null
}
```

失敗時は`ok: false`、`data: null`とし、`error`に安定した`code`、説明用`message`、必要に応じた`details`を含める。初期エラー識別子には`INVALID_ARGUMENT`、`INPUT_NOT_FOUND`、`INVALID_INPUT`、`OUTPUT_EXISTS`、`DEPENDENCY_MISSING`、`PROCESS_FAILED`、`TIMEOUT`、`IO_ERROR`を設ける。

文言の完全一致を機械判定に使わせない。共通エンベロープの破壊的変更はスキーマバージョンで区別する。パスは結果内では絶対パスを返す。

### 発見と診断

`list`は既知のCapability IDと説明を列挙する。`doctor`は利用可否、依存ツールの場所・バージョン、確認した範囲、利用できない理由を返す。

実行ファイルが存在するだけで、すべての入力形式を利用可能と断定しない。状態は`ready`、`missing`、`unsupported`、`unknown`を基本とする。診断処理自体が完了すれば、任意の外部依存がmissingでもdoctorは終了コード0とし、可否は結果で判断する。実際の変換で依存が不足した場合は非0とする。

## 7. 画像変換

初期コマンド:

```bash
foundry image webp input.png
foundry image webp input.jpg --output output.webp
foundry image webp input.png --lossless
foundry image webp input.jpg --quality 80
```

- JPG / JPEG / PNGの静止画を対象とし、アニメーション入力は明示的に対象外として扱う。
- 出力省略時は入力と同じフォルダ・同じstemの`.webp`を作る。
- 元画像を保持し、出力が存在すれば失敗する。MVPでは暗黙の上書き・元画像削除を提供しない。
- 標準は非可逆圧縮とするが、品質の初期値は写真・文字を含む画像で評価して確定する。`--quality`と`--lossless`の同時指定は引数エラー。
- `--lossless`はデコード後のピクセルに対する保証であり、元ファイルや全メタデータの保存を意味しない。
- EXIF orientation、透過、色プロファイル、メタデータの扱いをcodec選定時に確定し、対応範囲と制限を公開する。
- 出力が元画像より大きい場合も明示的な変換は完了させ、サイズ増加を警告する。軽量化を保証する表示はしない。
- 完成するまで最終出力を公開せず、失敗時に不完全なWebPを残さない。同時実行時も既存出力を保護する。
- 異常に大きい画像の寸法・デコード資源に上限を設ける。具体値は実測で決める。

まず単一入力を完成させる。複数入力、resize、optimize、PNG/JPEG出力は後続拡張とする。

「瞬時」は検証する目標であり現時点の保証ではない。プロセス起動を含む変換時間をrelease buildで測定し、画像サイズと実行環境を併記する。

## 8. スクリーンショット自動変換

初期はmacOSの保存先フォルダを対象とする。OS側の自動処理が保存を検知し、同じCLIを起動する。Foundry独自daemonは作らない。具体的なOS連携方式は検証後に一つ選ぶ。

対象は指定フォルダ内の新規JPG / PNGであり、クリップボードだけに保存した画像は含まない。スクリーンショット専用フォルダを推奨し、他の画像との区別を曖昧なファイル名判定だけに依存させない。

自動化側は書き込み完了待ち、重複イベントの抑止、変換並列数の制限を担当する。生成したWebPは監視対象から除外する。元画像を残し、失敗時には診断を確認できるようにする。既存WebPとの衝突は上書きせず記録する。設定の導入・解除手順を対で用意する。

スクリーンショット向け設定は文字品質を優先して可逆圧縮から評価する。圧縮率を優先する設定は非可逆圧縮として明示的に選べるようにする。

## 9. 後続Capability

MarkItDown Adapterは`foundry document markdown`を提供する。外部依存の検出、タイムアウト、終了コード、標準出力・標準エラーの捕捉を共通CLIへ統合する。初期は出力ファイルを明示し、本文とJSON結果の混在を避ける。

Officeは`pack` / `unpack`を先行する。未編集エントリの内容保持、必要なパーツの存在確認、ZIP展開先からの逸脱防止、展開量の制限を基本とする。XMLの自動整形は行わない。

ZIP再構築、パッケージ構造の検査、OOXMLスキーマ検証、Officeでの表示確認は別の保証とする。初期の構造検査を完全な`validate`と呼ばない。暗号化・署名付き文書などは対象範囲を明示し、黙って保証対象へ含めない。

## 10. 非目標と設計変更の境界

GUI、Web UI、HTTP API、MCPサーバー、独自daemon、データベース、認証、クラウドサービス、Plugin marketplace、独自言語・パッケージマネージャーは初期対象外。

機能に必要なライブラリは、ライセンス、保守状況、ビルド負荷、対応形式を確認して選定できる。将来のためだけのframework導入は行わない。元画像削除、全プロジェクトのRust環境変更、常駐サービス化など、データ保持や運用範囲を変える拡張は本計画とは別の判断として扱う。
