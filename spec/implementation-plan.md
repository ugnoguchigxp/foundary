# Foundry 実装計画

状態: 画像MVPとRust共通ビルド環境の初期基盤を実装済み。2026-09-21更新。

## 1. 到達目標

最初のリリースでは、プロジェクト外にインストールしたFoundryでJPG / PNGをWebP化できる状態を作る。次にmacOSのスクリーンショット保存から自動変換までを接続する。共通CLIの有効性を確認してから、外部AdapterとOfficeへ広げる。

設計規約は[プロジェクト構成](project-structure.md)を正とする。本書は実装順序と完了判定を定める。画像MVP、workspace、CIに加え、複数のTauri / Rustリポジトリで利用する共通ビルド環境の初期基盤まで実装済みである。

実装済みのRust環境基盤は`doctor rust`、`cache status`、`setup rust`、`env rust`、`run rust`で構成する。リポジトリの`foundry-rust.toml`を検証し、project、checkout、build unit、host tripleごとの外部targetを割り当て、RustとC/C++のコンパイルを共有sccacheへ接続する。グローバルCargo設定、Cargo.lock、Tauri設定、最終成果物は変更しない。

## 2. フェーズ0: 判断と検証素材

対象: `spec/`、`tests/fixtures/`。

- 初期macOS・CPU対象と開発用Rustバージョンを決める。
- WebP encoder / decoder候補を比較し、非可逆・可逆圧縮、EXIF orientation、透過、色、メタデータ方針を決める。
- Rustのみで完結させることを優先条件にしない。ネイティブ依存も含め、速度、ビルド負荷、配布サイズ、ライセンスを比較する。
- 写真JPEG、文字入りスクリーンショットPNG、透過PNG、orientation付きJPEG、破損入力を用意し、出所と利用条件を記録する。
- release buildで評価する代表画像群と測定環境を決める。未測定の処理時間を保証値として記載しない。

完了条件: codec選定理由、対応範囲、標準品質、資源上限、検証用画像が揃う。実装に関係しない将来機能の比較は行わない。

## 3. フェーズ1: 共通CLIとRust workspace

対象: ルートCargo設定、`foundry-cli`、`foundry-core`、基本CI。

- workspace、共通依存宣言、Cargo.lock、ツールチェーン設定を作る。
- `--help`、`--version`、`list`、`doctor`を実装する。
- 静的Capability定義、JSONエンベロープ、エラー識別子、終了コードを実装する。
- workerの配置解決とバージョン照合の最小契約を作る。
- 未実装のCapabilityをreadyと表示しない。
- CLIはテストから子プロセスとして起動できる構成にする。
- fmt、clippy、test、release buildをCIへ組み込む。

完了条件: 人間向け出力とJSON出力が正常系・引数エラーで検証される。doctorが機能不足と診断自体の失敗を区別する。CIが固定した依存解決で通る。

## 4. フェーズ2: 画像変換と配布 — 最初の実用MVP

対象: `foundry-image`、CLIの画像dispatch、fixtures、配布用scripts、README。

- 単一のJPG / PNG入力、出力先省略、`--output`、`--lossless`、`--quality`を実装する。
- 元画像保持、出力衝突時の失敗、一時出力と安全な確定処理を実装する。
- 入出力バイト数とサイズ増加警告を返す。
- 透過・orientation・色の扱いを検証し、制限をhelpとREADMEへ反映する。
- 公開CLIと画像workerを同一リリースでインストールする最小手順を作る。
- プロジェクト外の作業ディレクトリから実行する。

完了条件:

- 生成ファイルを独立にデコードでき、寸法・透過が仕様に合う。
- 可逆圧縮では仕様で定めた正規化後のピクセルが保持される。
- 非可逆圧縮では写真と文字の品質を確認し、初期品質の根拠を記録する。
- エラー時に元画像や既存出力が変化せず、不完全な最終出力を残さない。
- 空白・日本語を含むパス、同時出力衝突、破損入力をCLIから検証できる。
- 実行先プロジェクトにCargo設定、target、Python環境が追加されない。
- 起動込みの処理時間、圧縮率、配布サイズを測定し、READMEに測定条件を併記する。

## 5. フェーズ3: スクリーンショット自動変換

対象: `integrations/macos/`、導入・解除ドキュメント。

- macOSの自動処理方式を検証し、一つの推奨手順に絞る。
- 専用保存先から新規画像を検知してFoundryを起動する。
- 保存完了待ち、同一入力の重複抑止、並列数制限、ログを実装する。
- WebPを対象外にし、監視ループを防ぐ。
- CLIの絶対パスを利用し、対話shellと自動処理のPATH差を避ける。
- 可逆圧縮を初期設定として品質とサイズを評価する。

完了条件: 実際のスクリーンショット保存からWebP生成まで確認できる。連続撮影、書き込み中ファイル、名前衝突、変換失敗でデータを失わない。設定解除後は新しい変換が発生しない。通常のCIではイベント処理を模擬入力で検証し、OS連携はmacOSで別途確認する。

## 6. フェーズ4: 最初の外部Adapter

対象: `adapters/markitdown/`、CLI、doctor、fixtures。

- `foundry document markdown input.docx --output output.md`を実装する。
- 実行ファイルの場所・バージョンを診断する。必要な形式の利用可否と単なる存在確認を区別する。
- subprocessの出力を捕捉し、JSONを汚染しない。
- 未導入、異常終了、タイムアウト、部分出力の扱いを検証する。
- 非対話実行を維持し、勝手なインストールや外部サービス利用を行わない。

完了条件: 外部ツールの有無にかかわらずCLIテストが通る。通常CIでは偽の実行ファイルを用いて失敗経路を再現し、実ツールのsmoke testは導入済み環境で分けて実行する。

## 7. フェーズ5: Officeパッケージ操作

対象: `foundry-office`、CLI、Office fixtures。

- DOCX / XLSX / PPTXの`unpack` / `pack`を実装する。
- 未編集パーツを保持し、自動整形や不要ファイルの黙った混入を避ける。
- 展開先の逸脱、シンボリックリンク経由の逸脱、展開量超過、名前衝突を拒否する。
- 必要なパーツの存在など、限定した構造検査を実装する。
- 暗号化・署名付きなどの対象外条件を明示する。

完了条件: 3形式で未編集round-tripのエントリ集合と各内容を比較できる。代表的な編集fixtureも再packでき、Officeアプリでの開封確認を別途記録する。ZIP操作成功だけで文書の完全な妥当性を主張しない。完全なvalidateは別フェーズとする。

## 8. 共通検証

workspace作成後は次を実行する。

| コマンド / 方法 | 目的と期待結果 | 失敗時の対応 |
| --- | --- | --- |
| `cargo fmt --all -- --check` | 書式が一致する | 整形して再確認 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 警告なし | 原因を修正し、安易に全体抑制しない |
| `cargo test --workspace --locked` | 単体・CLIテストが通る | 該当fixtureと失敗条件を切り分ける |
| `cargo build --workspace --release --locked` | 配布対象をビルドできる | 依存・OS条件を確認 |
| インストール先からのsmoke test | リポジトリ外でworkerを解決できる | 配置・バージョン・PATHを修正 |
| release版の反復計測 | 起動込みの速度とサイズ効果を確認 | codec設定と支配的な処理を実測で見直す |

性能は代表画像ごとに複数回計測し、中央値と遅い側の値を記録する。OS・CPU、Foundryとcodecのバージョン、画像寸法、品質設定を併記する。共有CIの時間閾値だけで性能を判定しない。

## 9. 更新と復旧

各フェーズを独立してレビューできる変更単位にする。配布更新では公開CLIとworkerを混在させず、失敗時は直前のリリースへ戻せるようにする。自動変換に問題が出た場合はOS側の自動処理を解除し、手動CLIへ戻す。元画像を保持するため、変換済みファイルからの復元を前提にしない。

## 10. Rust共通ビルド環境の次段階

- profile manifestにtool version、SDK fingerprint、checksumを追加し、revision切替とrollbackをコマンド化する。
- project key単位のtarget一覧、容量上限、限定pruneを追加する。全targetやCargo download cacheの一括削除は提供しない。
- system native dependencyが実際に必要になった時点で、checksum付きimmutable prefixを一依存ずつ導入する。bundled SQLiteとlibwebpは強制的にsystem linkへ変えない。
- macOS arm64以外はhost triple別の受入試験を追加してから対応済みとする。

## 11. 後回しにするもの

複数画像の一括指定、resize、その他画像形式、XML / JSON整形、Hash、汎用アーカイブ、完全なOffice validate、他OSの自動化は、最初の利用実績を確認して優先順位を決める。

機能数ではなく、実際の複数プロジェクトでセットアップ回数、Agentの再試行数、処理時間、追加された依存が減ることを評価する。
