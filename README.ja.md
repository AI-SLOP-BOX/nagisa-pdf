# Nagisa PDF - デスクトップ＆モバイル PDFスイート

100%ローカル・オフライン処理の高性能でプロフェッショナルグレードの統合PDFエディター。Tauri v2、Rust、Reactで構築。

[English](README.md) | [日本語](README.ja.md)

---

## 機能

### コアPDF操作（110以上の操作）
- 結合、分割、削除、回転、並べ替え、複製、抽出
- テキスト挿入、画像挿入、動的オーバーレイ
- 切り抜き、トリム、余白調整
- 透かし、ヘッダー、フッター、ページ番号
- ブックマーク、ベーツナンバリング
- ドキュメント最適化、破損PDFの修復＆リカバリ
- PDF/Aアーカイブ変換
- パスワード保護、権限フラグ

### テキスト直接編集
- PDFストリーム内のテキストブロックを直接選択・編集
- テキスト段落の移動、再流し、削除
- フォントの自動検出とマッチング
- タイポグラフィ整形：色、サイズ、ベースライン、配置
- バウンディングボックスをまたぐテキストの再流し

### 注釈＆レビュー
- ハイライト、下線、波線、取り消し線
- スティッキー付箋、フリーハンド手書き
- ベクター図形：長方形、円、矢印、直線
- カスタムスタンプ＆ステータスバッジ
- 階層付き返信のコメントスレッド
- レビューワークフローのステータス追跡（承認、却下、完了）
- XFDFのインポート＆エクスポート

### インタラクティブフォーム
- フォーム作成＆フィールドレイアウト
- 計算フィールド＆自動算術
- フォームデータ収集＆サマリーエクスポート
- チェックボックス、ラジオグループ、ドロップダウンコンボ
- デジタル署名フィールド

### セキュリティ＆コンプライアンス
- AES-128 / AES-256 ドキュメント暗号化＆権限フラグ
- ビジュアル署名フィールドライアウト＆署名プレースホルダー管理（暗号学的PKCS#7署名は予定）
- ディープレダクション（ストリームのトークン化解除＆テキストトークンの物理削除）
- ドキュメントサニタイゼーション（メタデータ、注釈、サムネイル、Names JavaScript/EmbeddedFilesの削除）

### 変換＆相互運用性
- PDF→高解像度画像（pdftoppm経由のPNG、JPEG）
- 画像→PDF
- HTML→印刷品質PDF（隔離された一時ワークスペース＆セキュアファイルサンドボックス）
- PDF→プレーンテキスト＆構造化CSV
- PDF/A & PDF/X 標準コンプライアンス

### OCR（光学文字認識）
- 高精度な多言語テキスト認識（Tesseract OCR CLI）
- 検索可能PDFの生成（スキャン画像XObject＋不可視の選択可能テキストオーバーレイ）
- EPUBデジタルブック変換
- レイアウト維持OCR再構築

### カラーマネジメント＆プレプレス
- RGB→CMYKカラースペース変換
- ICC出力イントェントの埋め込み
- CMYKペイント演算のインクカバー検査＆閾値検証
- プリフライト検証（FontDescriptor間接参照追跡、画像DPI＆ICC検出）
- カラーセパレーションプレビュー

### アドバンスドエンジニアリング＆フォレンジックス
- 構造PDF検査＆オブジェクトツリー可視化
- 透明化フラットテニング
- PDF/UAアクセシビリティコンプライアンス検証
- ドキュメント差分比較（ビジュアル＆テキスト差分）
- 自動スキュー角度検出＆裏抜けシャドウ除去

---

## ダウンロード

開発環境（Node.js、Rust、Docker）は不要です。[Releases](https://github.com/AI-SLOP-BOX/nagisa-pdf/releases) ページからお使いのOS向けのビルド済みバイナリをダウンロードしてください：

| プラットフォーム | 形式 | インストール |
| :--- | :--- | :--- |
| **macOS** (Apple Silicon / Intel) | `.dmg` | [Releases](https://github.com/AI-SLOP-BOX/nagisa-pdf/releases) から `.dmg` をダウンロードし、`Applications` へドラッグして起動。 |
| **Android** (モバイル / タブレット) | `.apk` | [Releases](https://github.com/AI-SLOP-BOX/nagisa-pdf/releases) から `.apk` をダウンロードしてタップでインストール。 |
| **Linux** (Ubuntu, Fedora, Arch など) | `.AppImage`, `.deb` | `.AppImage` をダウンロードし、実行権限を付与（`chmod +x`）してそのまま実行。 |
| **Windows** (10 / 11) | `.exe` | [Releases](https://github.com/AI-SLOP-BOX/nagisa-pdf/releases) から `.exe` をダウンロードし、インストーラまたはポータブル実行ファイルを起動。 |

### 1行インストールスクリプト（macOS / Linux）
```bash
curl -fsSL https://raw.githubusercontent.com/AI-SLOP-BOX/nagisa-pdf/main/install.sh | bash
```

---

## ソースからビルド

### 前提条件
- [Node.js](https://nodejs.org/)（v18以上）
- [Rust](https://www.rust-lang.org/tools/install)（stableツールチェーン）
- [Tauri CLI](https://tauri.app/)（v2）

### macOS
```bash
# Homebrewでシステムツールをインストール
brew install node rust poppler tesseract tesseract-lang

# リポジトリをクローン
git clone https://github.com/AI-SLOP-BOX/nagisa-pdf.git
cd nagisa-pdf

# 依存関係をインストール
npm install

# 開発モードを実行
npx tauri dev
```

### Linux (Ubuntu / Debian)
```bash
sudo apt update
sudo apt install -y build-essential curl wget file libssl-dev libgtk-3-dev \
    libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf \
    poppler-utils tesseract-ocr tesseract-ocr-jpn

git clone https://github.com/AI-SLOP-BOX/nagisa-pdf.git
cd nagisa-pdf
npm install
npx tauri dev
```

### Linux (Fedora / RHEL)
```bash
sudo dnf install -y gcc gcc-c++ make openssl-devel gtk3-devel \
    webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel patchelf \
    poppler-utils tesseract tesseract-langpack-jpn

git clone https://github.com/AI-SLOP-BOX/nagisa-pdf.git
cd nagisa-pdf
npm install
npx tauri dev
```

### Windows
```bash
# Chocolateyでインストール
choco install nodejs rust poppler tesseract

# Visual Studio C++ Build Toolsが必要です：
# https://visual.microsoft.com/visual-cpp-build-tools

git clone https://github.com/AI-SLOP-BOX/nagisa-pdf.git
cd nagisa-pdf
npm install
npx tauri dev
```

---

## 本番ビルド

```bash
# 本番デスクトップアプリをコンパイル＆パッケージ
npx tauri build
```

コンパイル済みバンドルの出力先：
- macOS: `src-tauri/target/release/bundle/dmg/`
- Windows: `src-tauri/target/release/bundle/nsis/` または `msi/`
- Linux: `src-tauri/target/release/bundle/appimage/` または `deb/`

---


## プロジェクト構成

```
nagisa-pdf/
├── .github/workflows/       # 品質チェック＆マルチプラットフォームビルドのCI/CDワークフロー
├── src/                     # React + TypeScript フロントエンド
│   ├── App.tsx              # メインビュールーター＆レスポンシブラウトレイアウト
│   ├── main.tsx             # アプリケーションブートストラップ
│   ├── types.ts             # 共有フロントエンド型定義
│   ├── components/          # 再利用可能なUIコンポーネント＆ツールパネル
│   │   ├── CommandPalette.tsx # ユニバーサルコマンドパレット（Cmd+K）
│   │   ├── PDFViewer.tsx    # コアPDFビューアコンテナ
│   │   ├── UIControls.tsx   # デザインシステムのプリミティブコンポーネント
│   │   └── ...              # 注釈、フォームなどの専用パネル
│   ├── views/               # メインビューモード：PDFEditorView, ScannerView, OCRView
│   └── utils/               # ヘルパー、エラー処理、国際化
└── src-tauri/               # Tauri v2 バックエンド＆Rust PDFエンジン
    ├── Cargo.toml           # Rustパッケージ設定
    ├── src/
    │   ├── lib.rs           # Tauriコマンド登録＆IPC境界
    │   ├── image_engine.rs  # スキャン強化、コントラスト、透視補正
    │   ├── ocr_engine.rs    # OCRパイプライン＆検索可能PDF作成
    │   └── pdf_engine/      # モジュラーRust PDF処理エンジン
    │       ├── common.rs    # コア低レベルlopdfヘルパー
    │       ├── redact.rs    # バイトレベル物理レダクション
    │       ├── forms.rs     # インタラクティブフォーム処理
    │       ├── security.rs  # 暗号化＆デジタル署名検証
    │       ├── pdf_x.rs     # プリフライト＆PDF/X標準コンプライアンス
    │       └── ...
```

---

## 検証＆テスト

Nagisa PDFには、PDF整合性、バイトレベルレダクション、検索可能PDF生成をカバーする自動テストスイートが含まれています：

```bash
# Rustユニットテストを実行
cargo test --manifest-path src-tauri/Cargo.toml --lib

# コード整形チェックを実行
cargo fmt --manifest-path src-tauri/Cargo.toml --check

# TypeScriptビルドを確認
npm run build
```

---

## ライセンス＆帰属

Nagisa PDFは [MIT License](LICENSE) でライセンスされたオープンソースソフトウェアです。

### サードパーティ＆派生コード
- **Tauri / Tao**: ウィンドウ管理レイヤーには [Tauri / Tao](https://github.com/tauri-apps/tao) から改変したパッチが含まれており、**Apache License 2.0** でライセンスされています。
- **pdfjs-dist**: PDFレンダリングは [Mozilla PDF.js](https://github.com/mozilla/pdf.js) が提供しており、**Apache License 2.0** でライセンスされています。
- **Poppler**: レンダリング＆ラスタライゼーションの相互運用にはPopplerユーティリティ（GPLv2/GPLv3）を使用しています。
- **Tesseract OCR**: 光学文字認識はTesseractエンジン（Apache License 2.0）が提供しています。

