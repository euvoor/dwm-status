PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
XDG_CONFIG_HOME ?= $(HOME)/.config
CONFIG_DIR ?= $(XDG_CONFIG_HOME)/dwm_status
CONFIG_FILE ?= $(CONFIG_DIR)/config.toml
BIN ?= dwm_status
RELEASE_BIN ?= target/release/$(BIN)

.PHONY: all install dev

all:
	cargo run --release -- --config ./config.toml

install:
	cargo build --release
	install -d "$(BINDIR)" "$(CONFIG_DIR)"
	install -m755 "$(RELEASE_BIN)" "$(BINDIR)/$(BIN)"
	if [ ! -f "$(CONFIG_FILE)" ]; then install -m644 config.toml "$(CONFIG_FILE)"; fi

dev:
	cargo watch \
		--clear \
		--quiet \
		--ignore target \
		--ignore Makefile \
		--ignore README.md \
		--shell "cargo run -- --config ./config.toml"
