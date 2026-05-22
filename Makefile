SHELL := /bin/sh

HOST ?= localhost
PORT ?= 1521
DB_USER ?= sample
PASSWORD ?= sample
SERVICE_NAME ?= XE
SCHEMA ?= SAMPLE
OUTPUT ?= ./oracode-out
CONCURRENCY ?= 4
ORACLE_CLIENT_DIR ?= $(CURDIR)/.oracle-client/instantclient

ORACODE_ARGS := --host $(HOST) --port $(PORT) --user $(DB_USER) --password $(PASSWORD) --service-name $(SERVICE_NAME) --schema $(SCHEMA) --output $(OUTPUT) --concurrency $(CONCURRENCY)

.PHONY: help run test build release clean oracle-up oracle-down oracle-logs

help:
	@echo "可用目標："
	@echo "  make run         使用本機/範例 Oracle 資料庫執行 oracode"
	@echo "  make test        執行 Rust 測試"
	@echo "  make build       建置除錯版本二進位檔"
	@echo "  make release     建置最佳化發行版本二進位檔"
	@echo "  make clean       移除 Cargo 建置產物"
	@echo "  make oracle-up   啟動本機 Oracle XE 容器"
	@echo "  make oracle-down 停止並移除本機 Oracle XE 容器"
	@echo "  make oracle-logs 追蹤本機 Oracle XE 日誌"

run:
	DYLD_LIBRARY_PATH="$(ORACLE_CLIENT_DIR)" LD_LIBRARY_PATH="$(ORACLE_CLIENT_DIR)" cargo run -- $(ORACODE_ARGS)

test:
	cargo test

build:
	cargo build

release:
	cargo build --release

clean:
	cargo clean

oracle-up:
	docker run -d --name oracode-oracle -p 1521:1521 -e ORACLE_PASSWORD=Oracle123 gvenzl/oracle-xe:21-slim

oracle-down:
	docker rm -f oracode-oracle

oracle-logs:
	docker logs -f oracode-oracle
