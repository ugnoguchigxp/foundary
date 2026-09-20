# Foundry

各プロジェクトに変換ライブラリや実行環境を持ち込まず、共通のCLIでファイル処理を使うためのローカルツール基盤です。人間とCoding Agentの両方が利用します。

現在のMVPでは、JPG / JPEG / PNGを単一のWebPへ変換できます。`list`と`doctor`はJSON出力に対応しています。

```bash
# JPG / PNGを同じフォルダのWebPへ変換
foundry image webp screenshot.png
foundry image webp photo.jpg

# ピクセルを保持する可逆圧縮
foundry image webp screenshot.png --lossless

# 利用できる機能と環境を確認
foundry list --json
foundry doctor --json
```

出力先を省略すると、入力と同じフォルダに同じファイル名の`.webp`を作成します。元画像は残り、既存出力は上書きしません。`--lossless`と`--quality`は同時に指定できません。

スクリーンショットの保存をOS側で検知して同じCLIを起動する自動変換は、次のフェーズです。

## 計画している機能

| 順序 | 機能 | 内容 |
| --- | --- | --- |
| 1 | 画像変換と共通CLI | JPG / PNG → WebP、結果JSON、list、doctor（実装済み） |
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
