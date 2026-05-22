# oracode

`oracode` 可將 Oracle 資料庫物件匯出成乾淨、穩定且適合 Git 版本控管的 DDL 檔案。

## 功能特色

- 每個 Oracle 物件輸出成一個 `.sql` 檔案。
- 使用穩定的輸出目錄，例如 `TABLE/`、`VIEW/`、`PACKAGE_SPEC/` 與 `PACKAGE_BODY/`。
- 設定 `DBMS_METADATA` 轉換參數：`SEGMENT_ATTRIBUTES = FALSE`、`SQLTERMINATOR = TRUE`、`PRETTY = TRUE` 與 `EMIT_SCHEMA = FALSE`。
- 額外清理 DDL 中的 `EDITIONABLE`、多餘空白行、簡單識別字引號，以及 SQL 關鍵字大小寫。
- 支援併發匯出，並在終端機顯示進度列。
- 內容未變更時會略過檔案重寫。

## 安裝

從原始碼安裝：

```sh
cargo install --path .
```

發布版本完成後，npm 使用者可以安裝輕量包裝套件：

```sh
npm i -g oracode
```

## Oracle 用戶端需求

Rust `oracle` 驅動程式使用 ODPI-C，執行時需要 Oracle Client 函式庫。
請安裝 Oracle Instant Client，並讓系統能找到該函式庫目錄：

- macOS：設定 `DYLD_LIBRARY_PATH`，或確認 Instant Client 目錄位於載入器搜尋路徑中。
- Linux：設定 `LD_LIBRARY_PATH=/path/to/instantclient`。
- Windows：將 Instant Client 目錄加入 `PATH`。

## 使用方式

```sh
oracode \
  --host db.example.com \
  --port 1521 \
  --user HR \
  --password "$ORACLE_PASSWORD" \
  --service-name ORCLPDB1 \
  --schema HR \
  --output ./oracode-out
```

若使用 SID 連線，請改用 `--sid XE` 取代 `--service-name`。
如果需要保留加上雙引號的識別字，請使用 `--keep-quotes`。

所有 CLI 選項也可以透過 `ORACODE_*` 環境變數提供，例如 `ORACODE_PASSWORD`。

## 技術文件

- [技術架構與實作說明](docs/TECHNICAL.md)
- [產品需求與開發藍圖](docs/PRD.md)
