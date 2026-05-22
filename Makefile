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
	@echo "Targets:"
	@echo "  make run         Run oracode against the local/sample Oracle database"
	@echo "  make test        Run Rust tests"
	@echo "  make build       Build debug binary"
	@echo "  make release     Build optimized release binary"
	@echo "  make clean       Remove Cargo build artifacts"
	@echo "  make oracle-up   Start local Oracle XE container"
	@echo "  make oracle-down Stop and remove local Oracle XE container"
	@echo "  make oracle-logs Tail local Oracle XE logs"

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
