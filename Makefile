PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
XDG_CONFIG_HOME ?= $(HOME)/.config
CONFIG_DIR ?= $(XDG_CONFIG_HOME)/dwm_status
CONFIG_FILE ?= $(CONFIG_DIR)/config.toml
XDG_STATE_HOME ?= $(HOME)/.local/state
LOG_FILE ?= $(XDG_STATE_HOME)/dwm_status/dwm_status.log
BIN ?= dwm_status

.PHONY: all install reinstall dev

all:
	cargo run --release -- --config ./config.toml

install:
	cargo build --release
	install -d "$(BINDIR)" "$(CONFIG_DIR)"
	install -m755 "target/release/dwm_status" "$(BINDIR)/$(BIN)"
	if [ ! -f "$(CONFIG_FILE)" ]; then install -m644 config.toml "$(CONFIG_FILE)"; fi

reinstall: install
	bash scripts/restart-installed.sh "$(BINDIR)/$(BIN)" "$(CONFIG_FILE)" "$(LOG_FILE)"

dev:
	cargo watch \
		--clear \
		--quiet \
		--ignore target \
		--ignore Makefile \
		--ignore README.md \
		--shell "cargo run -- --config ./config.toml"
