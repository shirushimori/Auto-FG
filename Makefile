.PHONY: all setup build install run run-get-links run-download clean help uninstall

APP_NAME = Ffast-auto-downloader
BIN_NAME = Auto-FG

all: build

setup:
	@./setup.sh

build:
	cargo build --release

install: build
	@mkdir -p $(HOME)/.local/bin
	@cp target/release/$(APP_NAME) $(HOME)/.local/bin/$(BIN_NAME)
	@cp target/release/get-links $(HOME)/.local/bin/$(BIN_NAME)-get-links
	@cp target/release/download $(HOME)/.local/bin/$(BIN_NAME)-download
	@chmod +x $(HOME)/.local/bin/$(BIN_NAME)
	@chmod +x $(HOME)/.local/bin/$(BIN_NAME)-get-links
	@chmod +x $(HOME)/.local/bin/$(BIN_NAME)-download
	@echo "Installed to ~/.local/bin."

uninstall:
	@rm -f $(HOME)/.local/bin/$(BIN_NAME) $(HOME)/.local/bin/$(BIN_NAME)-get-links $(HOME)/.local/bin/$(BIN_NAME)-download
	@rm -f $(HOME)/.local/share/applications/$(BIN_NAME).desktop
	@echo "Uninstalled."

run: build
	cargo run --release

run-get-links: build
	cargo run --release --bin get-links

run-download: build
	cargo run --release --bin download

clean:
	cargo clean

help:
	@echo "Targets:"
	@echo "  setup          Full setup via setup.sh"
	@echo "  build          Release build"
	@echo "  install        Build + copy to ~/.local/bin"
	@echo "  uninstall      Remove installed files"
	@echo "  run            Build + run GUI"
	@echo "  run-get-links  Build + run get-links CLI"
	@echo "  run-download   Build + run download CLI"
	@echo "  clean          Remove build artifacts"
