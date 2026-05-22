# `oracode` 技術文件

本文說明 `oracode` 的整體架構、技術棧、資料流程、模組責任、Oracle 匯出策略，以及本機開發與發布設計。

## 專案定位

`oracode` 是一個以 Rust 撰寫的 Oracle DDL 匯出工具，目標是把資料庫 Schema 轉換成穩定、乾淨、適合 Git 版本控管的 SQL 檔案。

核心設計重點：

- **Database as Code：** 將 Oracle Schema 物件以檔案形式保存，讓資料庫結構可以被審查、比對與版本控管。
- **穩定輸出：** 移除環境相關與容易產生雜訊的 DDL 內容，降低不必要的 `git diff`。
- **一物件一檔案：** 每個 Oracle 物件輸出成獨立 `.sql`，依物件類型分目錄保存。
- **可重複執行：** 未變更的檔案不會重寫，避免修改時間與版本差異被污染。
- **跨平台發布：** 透過 `cargo-dist` 規劃跨平台二進位檔與 npm 包裝層。

## 技術棧

| 技術 | 用途 |
| --- | --- |
| Rust 2024 Edition | 主要實作語言，提供型別安全、效能與單一二進位檔發布能力 |
| `clap` | CLI 參數解析、環境變數支援與互斥參數驗證 |
| `tokio` | 非同步執行、檔案 I/O、任務排程與併發控制 |
| `oracle` | Oracle Database 驅動程式，透過 ODPI-C 連線與執行 SQL |
| ODPI-C / Oracle Instant Client | Oracle 原生用戶端函式庫，執行時由 `oracle` crate 載入 |
| `regex` | DDL 文字清理，例如移除 `EDITIONABLE`、處理簡單識別字引號 |
| `indicatif` | 終端機進度列 |
| `thiserror` | 統一錯誤型別與錯誤訊息格式 |
| `cargo-dist` | 發行流程、跨平台建置與 npm 包裝規劃 |
| Docker | 本機 Oracle XE 測試資料庫 |
| Makefile | 常用開發命令封裝 |

## 目錄結構

```text
.
├── Cargo.toml          # Rust 套件資訊、依賴與 cargo-dist 設定
├── Makefile            # 常用開發、建置、測試與本機 Oracle 容器命令
├── README.md           # 使用者入口文件
├── docs/
│   ├── PRD.md          # 產品需求與開發藍圖
│   └── TECHNICAL.md    # 技術架構文件
├── npm/                # npm 包裝層相關檔案
├── src/
│   ├── main.rs         # CLI 進入點
│   ├── lib.rs          # crate 模組匯出
│   ├── cli.rs          # CLI 參數、連線設定與 connect descriptor 產生
│   ├── db.rs           # Oracle 連線、物件列舉與 DBMS_METADATA DDL 擷取
│   ├── export.rs       # 匯出流程、併發控制、檔案寫入與進度列
│   ├── sanitize.rs     # DDL 淨化與格式穩定化
│   └── error.rs        # 統一錯誤型別
└── tests/              # 整合測試預留位置
```

## 執行流程總覽

`oracode` 的主要流程如下：

```text
使用者輸入 CLI 參數
        │
        ▼
clap 解析參數與 ORACODE_* 環境變數
        │
        ▼
建立 Oracle connect descriptor
        │
        ▼
連線 Oracle，設定 DBMS_METADATA transform
        │
        ▼
查詢指定 Schema 的支援物件清單
        │
        ▼
依 concurrency 併發呼叫 DBMS_METADATA.GET_DDL
        │
        ▼
清理 DDL 文字並穩定格式
        │
        ▼
依物件類型輸出到對應目錄
        │
        ▼
若檔案內容未變更則略過寫入
```

## 模組設計

### `src/main.rs`

CLI 進入點，責任很薄：

1. 使用 `clap::Parser` 解析命令列參數。
2. 呼叫 `oracode::run(cli)` 執行匯出流程。
3. 若發生錯誤，將錯誤輸出到 `stderr` 並以非零狀態碼結束。

這種設計讓主程式維持簡單，核心邏輯集中於 library crate，方便測試與未來重用。

### `src/lib.rs`

集中宣告並匯出內部模組：

- `cli`
- `db`
- `error`
- `export`
- `sanitize`

也重新匯出 `Cli`、`Error`、`Result` 與 `run`，讓 binary crate 可以用簡潔介面呼叫 library。

### `src/cli.rs`

負責 CLI 參數與連線設定。

主要功能：

- 定義 `Cli` 結構，對應所有命令列參數。
- 支援 `ORACODE_*` 環境變數，例如 `ORACODE_HOST`、`ORACODE_PASSWORD`。
- 使用 `ArgGroup` 確保 `--sid` 與 `--service-name` 必須擇一提供。
- 將 CLI 參數轉換成 `ConnectionConfig`。
- 產生 Oracle connect descriptor。

連線描述字範例：

```text
(DESCRIPTION=(ADDRESS=(PROTOCOL=TCP)(HOST=localhost)(PORT=1521))(CONNECT_DATA=(SERVICE_NAME=XE)))
```

設計考量：

- 不依賴本機 `tnsnames.ora`，降低部署環境差異。
- 由程式直接組出 descriptor，方便跨平台與容器環境使用。
- `--sid` 與 `--service-name` 明確互斥，避免產生模糊連線設定。

### `src/db.rs`

負責 Oracle 互動。

主要型別：

- `ObjectKind`：抽象化支援的 Oracle 物件類型。
- `DbObject`：代表待匯出的資料庫物件，包含名稱與物件類型。
- `OracleMetadataClient`：封裝 Oracle 連線、Metadata 設定與 DDL 擷取。

支援的物件類型：

- `TABLE`
- `VIEW`
- `PROCEDURE`
- `FUNCTION`
- `PACKAGE`
- `PACKAGE BODY`
- `TRIGGER`
- `SEQUENCE`
- `TYPE`
- `TYPE BODY`

#### Metadata transform 設定

建立連線後會設定：

```sql
DBMS_METADATA.SET_TRANSFORM_PARAM(DBMS_METADATA.SESSION_TRANSFORM, 'SEGMENT_ATTRIBUTES', FALSE);
DBMS_METADATA.SET_TRANSFORM_PARAM(DBMS_METADATA.SESSION_TRANSFORM, 'SQLTERMINATOR', TRUE);
DBMS_METADATA.SET_TRANSFORM_PARAM(DBMS_METADATA.SESSION_TRANSFORM, 'PRETTY', TRUE);
DBMS_METADATA.SET_TRANSFORM_PARAM(DBMS_METADATA.SESSION_TRANSFORM, 'EMIT_SCHEMA', FALSE);
```

這些設定的目的：

- `SEGMENT_ATTRIBUTES = FALSE`：移除 tablespace、storage 等環境相依資訊。
- `SQLTERMINATOR = TRUE`：輸出 SQL 結尾分號。
- `PRETTY = TRUE`：讓 Oracle 產生較可讀的 DDL。
- `EMIT_SCHEMA = FALSE`：避免輸出 schema 前綴，提升跨環境可攜性。

#### 物件清單查詢

工具會從 `ALL_OBJECTS` 讀取指定 Schema 的支援物件。查詢時會排除：

- 資源回收桶物件：`BIN$%`
- Oracle identity 欄位自動產生的 sequence

排除 identity sequence 的原因是這類 sequence 由 Oracle 內部管理，常見名稱如 `ISEQ$$_...`。直接對它呼叫 `DBMS_METADATA.GET_DDL` 可能產生 `ORA-31603`，而且 identity sequence 本身已包含在 table DDL 語意中，不應作為獨立業務物件輸出。

### `src/export.rs`

負責匯出協調、併發控制、進度顯示與檔案寫入。

主要流程：

1. 驗證 `--concurrency` 必須大於 0。
2. 建立連線並取得物件清單。
3. 建立進度列。
4. 使用 `tokio::sync::Semaphore` 控制最大併發數。
5. 使用 `tokio::task::JoinSet` 管理每個物件的匯出任務。
6. 對每個物件建立獨立 Oracle 連線並呼叫 `GET_DDL`。
7. 清理 DDL。
8. 寫入對應檔案。
9. 統計寫入與略過數量。

#### 為什麼使用 `spawn_blocking`

Oracle driver 的連線與查詢屬於阻塞式操作。為避免阻塞 Tokio runtime 的非同步工作執行緒，資料庫操作會放入 `tokio::task::spawn_blocking`。

這樣可以讓：

- 阻塞式 Oracle I/O 與非同步檔案 I/O 分離。
- Tokio runtime 不會因資料庫查詢而卡住。
- 多物件匯出仍可透過併發提升吞吐量。

#### 併發模型

`--concurrency` 控制同時匯出的物件數。預設值在 CLI 中為 `8`，Makefile 的本機範例預設為 `4`。

併發太高可能造成：

- Oracle session 數量過多。
- 本機 Instant Client 或資料庫容器負載升高。
- 網路或資料庫端節流。

建議依資料庫規模與環境調整，例如：

```sh
oracode ... --concurrency 4
oracode ... --concurrency 16
```

#### 檔案寫入策略

寫入前會先讀取既有檔案內容：

- 若內容相同：回傳 `Skipped`，不重寫檔案。
- 若內容不同：覆寫檔案。
- 若檔案不存在：建立新檔案。

這可以避免因重複匯出造成大量不必要的檔案修改。

### `src/sanitize.rs`

負責 DDL 淨化與格式穩定化。

目前清理規則：

- 移除 `EDITIONABLE` / `NONEDITIONABLE`。
- 將簡單雙引號識別字改成未加引號形式，例如 `"EMPLOYEES"` 變成 `EMPLOYEES`。
- 若使用 `--keep-quotes`，則保留雙引號識別字。
- 將 SQL 關鍵字正規化為大寫。
- 保留字串常值、雙引號內容與註解中的原始文字。
- 移除行尾空白。
- 將過多空白行壓縮為最多一個空白行。
- 確保輸出結尾有單一換行。

#### 關鍵字正規化

Sanitizer 會逐字元掃描 DDL，而不是單純用全域替換，原因是必須避免改到：

- 單引號字串，例如 `'select from'`
- 雙引號識別字，例如 `"MixedCaseName"`
- 單行註解，例如 `-- where stays lowercase`
- 區塊註解，例如 `/* select */`

這能在維持輸出穩定性的同時，降低破壞 SQL 語意的風險。

### `src/error.rs`

使用 `thiserror` 定義統一錯誤型別。

錯誤來源包含：

- Oracle 錯誤
- 檔案 I/O 錯誤
- Tokio task join 錯誤
- 不支援的 Oracle 物件類型
- 無效併發設定
- 單一物件匯出失敗

單一物件匯出失敗時會包裝物件類型與名稱，例如：

```text
failed to export TABLE EMPLOYEES: ...
```

這讓使用者可以快速定位失敗物件。

## 輸出目錄規則

每個物件依 `ObjectKind::metadata_type()` 對應到固定目錄：

| Oracle 物件 | 輸出目錄 |
| --- | --- |
| TABLE | `TABLE/` |
| VIEW | `VIEW/` |
| PROCEDURE | `PROCEDURE/` |
| FUNCTION | `FUNCTION/` |
| PACKAGE | `PACKAGE_SPEC/` |
| PACKAGE BODY | `PACKAGE_BODY/` |
| TRIGGER | `TRIGGER/` |
| SEQUENCE | `SEQUENCE/` |
| TYPE | `TYPE_SPEC/` |
| TYPE BODY | `TYPE_BODY/` |

檔名會使用物件名稱，並將不適合檔名的字元替換成 `_`。

範例：

```text
oracode-out/
├── TABLE/
│   ├── DEPARTMENTS.sql
│   └── EMPLOYEES.sql
├── VIEW/
│   └── EMPLOYEE_DIRECTORY.sql
├── PACKAGE_SPEC/
│   └── HR_REPORT.sql
└── PACKAGE_BODY/
    └── HR_REPORT.sql
```

## Oracle Client 執行時需求

Rust `oracle` crate 透過 ODPI-C 載入 Oracle Instant Client，因此執行 `oracode` 的機器必須能找到 Oracle Client 動態函式庫。

常見設定：

```sh
# macOS
export DYLD_LIBRARY_PATH=/path/to/instantclient

# Linux
export LD_LIBRARY_PATH=/path/to/instantclient

# Windows PowerShell
$env:PATH = "C:\path\to\instantclient;$env:PATH"
```

本專案的 Makefile 預設使用：

```text
.oracle-client/instantclient
```

該目錄僅供本機開發使用，已被 `.gitignore` 忽略，不應提交到版本庫。

## 本機 Oracle XE 開發環境

可透過 Docker 啟動本機 Oracle XE：

```sh
make oracle-up
```

預設容器設定：

| 項目 | 值 |
| --- | --- |
| 容器名稱 | `oracode-oracle` |
| 映像檔 | `gvenzl/oracle-xe:21-slim` |
| 本機連接埠 | `1521` |
| SYS 密碼 | `Oracle123` |
| 服務名稱 | `XE` |

查看日誌：

```sh
make oracle-logs
```

停止並移除容器：

```sh
make oracle-down
```

目前範例 Schema 使用：

| 項目 | 值 |
| --- | --- |
| 使用者 | `sample` |
| 密碼 | `sample` |
| Schema | `SAMPLE` |
| 服務名稱 | `XE` |

## Makefile 開發命令

常用命令：

```sh
make help
make run
make test
make build
make release
make clean
```

`make run` 會自動設定本機 Oracle Client 路徑：

```sh
DYLD_LIBRARY_PATH="$(ORACLE_CLIENT_DIR)" LD_LIBRARY_PATH="$(ORACLE_CLIENT_DIR)" cargo run -- ...
```

可覆寫參數：

```sh
make run DB_USER=sample PASSWORD=sample SCHEMA=SAMPLE OUTPUT=./oracode-out CONCURRENCY=4
```

## 測試策略

目前測試以單元測試為主，涵蓋：

- Oracle connect descriptor 產生。
- Oracle object type 與 metadata type 對應。
- 檔名清理。
- 檔案內容未變更時略過寫入。
- DDL sanitizer 的主要規則。

執行測試：

```sh
make test
```

或直接執行：

```sh
cargo test
```

未來可補強的測試類型：

- 使用 Docker Oracle XE 的整合測試。
- 不同 Oracle 版本的 DDL 輸出快照測試。
- 大型 Schema 的壓力測試。
- 特殊命名、保留字、混合大小寫識別字測試。

## 發布設計

`Cargo.toml` 已設定 `cargo-dist` metadata，規劃目標平台包含：

- `aarch64-apple-darwin`
- `x86_64-apple-darwin`
- `x86_64-unknown-linux-gnu`
- `x86_64-pc-windows-msvc`

發布目標：

- GitHub Release 上傳跨平台二進位檔。
- 提供 shell / PowerShell 安裝腳本。
- 透過 npm 發布薄包裝層，讓使用者可以用 `npm i -g oracode` 安裝。

注意：即使 `oracode` 本身可用單一二進位檔發布，目標機器仍須安裝 Oracle Instant Client，因為 Oracle 驅動程式執行時需要原生 client library。

## 設計限制與注意事項

### Oracle 權限

匯出指定 Schema 時，使用者需要能讀取 `ALL_OBJECTS` 與透過 `DBMS_METADATA.GET_DDL` 取得物件 DDL。

若匯出非自身 Schema，可能需要額外權限，例如：

- 讀取目標 Schema 物件。
- 使用 `DBMS_METADATA` 取得 DDL。
- 存取 package body、type body 等物件內容。

### `DBMS_METADATA` 的輸出差異

不同 Oracle 版本可能產生略有不同的 DDL。`oracode` 會盡量清理常見雜訊，但仍應預期：

- Oracle 版本差異可能造成輸出格式差異。
- 某些進階物件屬性仍可能包含版本或環境相關內容。
- 特殊物件類型可能需要額外 sanitizer 規則。

### 識別字引號

預設會移除簡單的大寫雙引號識別字，讓輸出更乾淨。

若 Schema 使用大小寫敏感名稱、保留字名稱或特殊字元名稱，建議使用：

```sh
oracode ... --keep-quotes
```

### 密碼處理

CLI 支援 `--password` 與 `ORACODE_PASSWORD`。在 shell history 敏感的環境中，建議使用環境變數：

```sh
export ORACODE_PASSWORD='...'
oracode --host ... --user ... --service-name ... --schema ...
```

## 未來可擴充方向

- 新增 include/exclude 物件類型參數。
- 新增 include/exclude 物件名稱 pattern。
- 新增 DDL snapshot 測試。
- 支援設定檔，例如 `oracode.toml`。
- 支援匯出 grants、synonyms、materialized views、database links 等更多物件。
- 支援產出 manifest，記錄每次匯出的物件、雜湊與時間。
- 支援 dry-run，比對資料庫與本機輸出差異但不寫入。
- 支援 JSON / SARIF 格式報告，方便 CI 使用。
- 加強 sanitizer pipeline，使各規則可設定、可測試、可獨立啟用。
