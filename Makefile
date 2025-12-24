# SR Matcher - Monorepo Makefile
# Rust (crates/) と GUI (gui/) の入口を提供

.PHONY: help rust-build rust-test rust-check api-dev gui-dev gui-build gui-lint gui-install all dev

help:
	@echo "SR Matcher Commands"
	@echo ""
	@echo "Rust:"
	@echo "  make rust-build   - Build all Rust crates"
	@echo "  make rust-test    - Run all Rust tests"
	@echo "  make rust-check   - Run cargo check"
	@echo ""
	@echo "API:"
	@echo "  make api-dev      - Start sr-api server (requires .env)"
	@echo ""
	@echo "GUI:"
	@echo "  make gui-install  - Install npm dependencies"
	@echo "  make gui-dev      - Start Vite dev server"
	@echo "  make gui-build    - Production build"
	@echo "  make gui-lint     - Run ESLint"
	@echo ""
	@echo "All:"
	@echo "  make all          - Build everything"
	@echo ""
	@echo "Quickstart:"
	@echo "  1. cp .env.example .env && edit DATABASE_URL/SR_API_KEY"
	@echo "  2. Terminal 1: make api-dev"
	@echo "  3. Terminal 2: make gui-dev"

# ============================================================
# Rust
# ============================================================

rust-build:
	cargo build --release

rust-test:
	cargo test

rust-check:
	cargo check

# ============================================================
# API (sr-api)
# ============================================================

api-dev:
	cargo run -p sr-api

# ============================================================
# GUI
# ============================================================

gui-install:
	cd gui && npm install

gui-dev:
	cd gui && npm run dev

gui-build:
	cd gui && npm run build

gui-lint:
	cd gui && npm run lint

# ============================================================
# All
# ============================================================

all: rust-build gui-build
