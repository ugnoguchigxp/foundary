# Foundry

各プロジェクトに変換ライブラリや実行環境を持ち込まず、共通のCLIでファイル処理を使うためのローカルツール基盤です。人間とCoding Agentの両方が利用します。

現在のMVPでは、JPG / JPEG / PNGを単一のWebPへ変換できます。`list`と`doctor`はJSON出力に対応しています。macOS上のRust/C/C++ビルド環境を診断し、プロジェクトごとに隔離したCargo targetと共有sccacheを使ってコマンドを実行できます。

```bash
# JPG / PNGを同じフォルダのWebPへ変換
foundry image webp screenshot.png
foundry image webp photo.jpg

# ピクセルを保持する可逆圧縮
foundry image webp screenshot.png --lossless

# 利用できる機能と環境を確認
foundry list --json
foundry doctor --json
foundry doctor rust --json
foundry cache status --json

# 現在のshellだけでcompiler cacheを有効化する
eval "$(foundry env rust)"

# いつでもsourceできるFoundry管理プロファイルを作成する
foundry setup rust

# foundry-rust.tomlに従い、専用targetと共有compiler cacheでbuildする
foundry run rust --project /path/to/project --unit workspace -- \
  cargo test --workspace --locked

# 同じ環境変数を確認する（evalすれば手動コマンドにも利用可能）
foundry env rust --project /path/to/project --unit workspace
```

出力先を省略すると、入力と同じフォルダに同じファイル名の`.webp`を作成します。元画像は残り、既存出力は上書きしません。`--lossless`と`--quality`は同時に指定できません。

スクリーンショットの保存をOS側で検知して同じCLIを起動する自動変換は、次のフェーズです。

## 計画している機能

| 順序 | 機能 | 内容 |
| --- | --- | --- |
| 1 | 画像変換と共通CLI | JPG / PNG → WebP、結果JSON、list、doctor（実装済み） |
| 1.5 | Rustビルド環境基盤 | rustup、Xcode SDK、clang、CMake、Ninja、pkg-config、sccache、Cargo cacheを診断し、隔離targetで実行（実装済み） |
| 2 | スクリーンショット自動変換 | macOS側の自動処理とCLIを接続（未実装） |
| 3 | 外部ツールAdapter | MarkItDownによるMarkdown変換 |
| 4 | Officeパッケージ操作 | DOCX / XLSX / PPTXのpack / unpack、限定的な構造検査 |

Rustで実装する機能と、既存CLIを呼ぶ機能を一つの入口へまとめます。画像処理などのRust依存とビルド成果物はFoundry側で管理し、利用するプロジェクトにはSDKやcrateを追加しません。

## インストール方針

初期対象はmacOSです。Rust stableで開発し、利用時にはビルド済みバイナリをユーザー環境のPATH上へ配置する方針です。利用者にRustのインストールは要求しません。インストール用の配布手順は未実装です。開発中は次で実行できます。

```bash
cargo run -p foundry-cli -- image webp path/to/image.png --json
```

外部ツールは別途導入します。Foundryは実行時に不足を診断し、暗黙のインストールやダウンロードは行いません。配布形式、正式な配置先、インストールコマンドは実装時に確定します。

`foundry cache status` は `CARGO_HOME`（未指定時は `~/.cargo`）のregistry/Git cache、Foundry管理のsccache、隔離target群を読むだけです。既存のCargoキャッシュを削除、移動、再構築することはありません。

`foundry env rust` はopt-inのzsh設定を出力します。引数なしでは`RUSTC_WRAPPER=sccache`、Foundry管理の`SCCACHE_DIR`（既定20GB）、`CARGO_INCREMENTAL=0`だけを設定します。`--project`を付けると、リポジトリの`foundry-rust.toml`を検証し、toolchain、C/C++ compiler、CMake launcher、プロジェクト・checkout・build unit・host tripleごとに隔離した`CARGO_TARGET_DIR`も加えます。グローバルな`RUSTFLAGS`、OpenSSL/SQLite/FFmpeg用の環境変数、`~/.cargo/config.toml`は変更しません。

`foundry run rust`は生成した環境を子プロセスだけへ渡します。`cc` crateは`RUSTC_WRAPPER`を認識してclangのコンパイルもsccacheへ載せ、CMakeにはcompiler launcherを明示します。SQLiteやlibwebpのbundled source、各リポジトリのbuild.rsとCargo.lockはそのままです。Foundryを使わない通常の`cargo`コマンドは常にfallbackとして残ります。

各リポジトリには次の最小宣言だけを置きます。複数のCargo.lockを持つリポジトリはbuild unitを分けます。

```toml
schema_version = 1
project_id = "example"
toolchain = "1.92.0"

[[build_units]]
id = "workspace"
manifest_path = "Cargo.toml"
```

`foundry setup rust` は `~/.config/foundry/rust-build.zsh`、共有cache directory、診断用profile manifestを作成・更新します。既存のshell初期化ファイルは変更しません。設定を持続させたい場合は、利用者が自分のshell設定からこのファイルをsourceします。Foundry管理外の同名ファイルは上書きしません。

## ドキュメント

- [プロジェクト構成と設計規約](spec/project-structure.md)
- [実装計画と完了条件](spec/implementation-plan.md)

## 開発

Rust workspace、CLI統合テスト、GitHub Actionsの基本CIを用意しています。開発時は次を実行します。

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

GUI、HTTP API、常駐サーバー、独自パッケージマネージャー、動的Plugin基盤は初期スコープに含めません。
