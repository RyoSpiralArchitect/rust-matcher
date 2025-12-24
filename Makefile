# SR Matcher - Monorepo Makefile
# Rust (crates/) と GUI (gui/) の入口を提供

.PHONY: help rust-build rust-test rust-check gui-dev gui-build gui-lint gui-install all

help:
	@echo "SR Matcher Commands"
	@echo ""
	@echo "Rust:"
	@echo "  make rust-build   - Build all Rust crates"
	@echo "  make rust-test    - Run all Rust tests"
	@echo "  make rust-check   - Run cargo check"
	@echo ""
	@echo "GUI:"
	@echo "  make gui-install  - Install npm dependencies"
	@echo "  make gui-dev      - Start dev server"
	@echo "  make gui-build    - Production build"
	@echo "  make gui-lint     - Run ESLint"
	@echo ""
	@echo "All:"
	@echo "  make all          - Build everything"

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
