# `oracode` 開發計畫：Oracle Database as Code CLI

## 核心設計理念

1. **純淨的 DDL：** 徹底移除環境綁定參數（Tablespace、Storage）與多餘雜訊（Editionable、無效空白），確保產出的 DDL 具有穩定格式。
2. **適合 Git 版本控管：** 單一物件單一檔案，目錄結構清晰，讓 `git diff` 只顯示真實的業務邏輯變更。
3. **安裝零摩擦：** 透過 `cargo-dist` 產生跨平台單一執行檔，並發布至 npm，開發者只需 `npm i -g oracode` 即可使用。
4. **高效能：** 使用 Rust 實作，利用併發處理大量物件的高速匯出。

---

## 階段開發藍圖

### 階段 1：專案基礎建設與 CI/CD 管線建立

這個階段的重點是把 DevOps 流程先架好，確保後續開發的每一行原始碼都能自動被測試與建置。

* **1.1 專案初始化與套件選擇：**
* 使用 `cargo new oracode` 建立專案。
* 核心依賴：`clap` (CLI 介面)、`tokio` (非同步)、`oracle` (資料庫驅動)、`regex` (字串處理)、`indicatif` (進度條)。


* **1.2 導入 `cargo-dist`：**
* 在專案中執行 `cargo dist init`，配置支援的目標平台（如 `x86_64-unknown-linux-gnu`、`x86_64-apple-darwin`、`aarch64-apple-darwin`、`x86_64-pc-windows-msvc`）。
* 設定安裝程式，包含 npm 套件與 shell script/PowerShell。


* **1.3 GitHub Actions 工作流程（CI/CD）：**
* **CI 管線（`ci.yml`）：** 處理 PR 與 Push 事件，包含 `cargo fmt --check`、`cargo clippy` 與 `cargo test`。
* **CD 管線（由 cargo-dist 產生的 `release.yml`）：** 攔截 Git Tag（例如 `v1.0.0`）事件，自動跨平台編譯、建立 GitHub Release、上傳二進位檔，並自動將包裝好的工具推送到 npm registry。



### 階段 2：核心功能實作 - Oracle 資料庫互動

處理與 Oracle 資料庫的連線，以及操作 `DBMS_METADATA` 取得初步去雜訊的資料。

* **2.1 CLI 參數解析 (`clap` 實作)：**
* 實作連線參數：`--host`, `--port`, `--user`, `--password`, `--sid` / `--service-name`。
* 實作目標參數：`--schema`（目標 Schema）、`--output`（輸出目錄，預設 `./oracode-out`）。


* **2.2 注入 `DBMS_METADATA` 參數 (第一道防線)：**
* 建立資料庫連線後，立刻執行參數設置：
* `SEGMENT_ATTRIBUTES = FALSE`
* `SQLTERMINATOR = TRUE`
* `PRETTY = TRUE`
* `EMIT_SCHEMA = FALSE`




* **2.3 物件清單擷取：**
* 從 `ALL_OBJECTS` 查詢指定 Schema 下支援的物件類型（TABLE、VIEW、PROCEDURE、FUNCTION、PACKAGE、TRIGGER、SEQUENCE 等）。



### 階段 3：核心功能實作 - DDL 淨化與格式穩定化

這是 `oracode` 的核心價值所在，處理 Oracle 內建功能無法解決的雜訊。

* **3.1 字串淨化器 (Rust Sanitizer)：**
* 移除 `EDITIONABLE` 關鍵字（使用 Regex）。
* 處理多餘的空行 (連續三個換行縮減為兩個)。
* 移除物件名稱外的雙引號 (例如 `"EMPLOYEES"` -> `EMPLOYEES`)，若有保留字需求需提供設定開關 `--keep-quotes`。


* **3.2 關鍵字正規化 (大寫強制轉換)：**
* 將 DDL 中的 SQL 關鍵字統一轉換為大寫，避免因手動修改資料庫導致的排版差異。



### 階段 4：系統 I/O 與併發處理

優化大量物件的匯出速度，並建立結構化的檔案系統。

* **4.1 目錄結構生成：**
* 依據物件類型自動建立子目錄，例如：`output/TABLE/`, `output/VIEW/`, `output/PACKAGE_SPEC/`, `output/PACKAGE_BODY/`。


* **4.2 併發執行與進度顯示：**
* 利用 `tokio::spawn` 併發呼叫 `GET_DDL` 並處理字串。
* 整合 `indicatif` 顯示終端機進度條 (例如：`[00:02:15] [####.......] 450/1200 objects exported`)。


* **4.3 檔案寫入優化 (可選)：**
* 在寫入檔案前，計算新產生的 DDL 的 Hash 值，並與磁碟上的現有檔案比較。如果內容相同則略過寫入，減少不必要的 I/O 與檔案修改時間變更。



### 階段 5：文件撰寫與首發準備

確保開發者能順利安裝與使用。

* **5.1 撰寫高品質的 `README.md`：**
* 介紹核心理念、安裝方式 (npm 或 shell 腳本)。
* **特別重要：** 由於依賴 Oracle Instant Client (或 ODPI-C)，必須提供清晰的環境設定指南 (如何設定 `LD_LIBRARY_PATH` 或 Windows 的環境變數)。


* **5.2 發布 v0.1.0：**
* 打上 Git Tag 觸發階段 1 建立好的 CD 管線，檢視 GitHub Release 與 npm 發布是否成功。
