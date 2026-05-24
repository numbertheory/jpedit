.PHONY: all build clean install

BINARY_NAME=jpedit
INSTALL_DIR=$(HOME)/.local/bin

all: build

build:
	cargo build --release

clean:
	cargo clean

install: build
	mkdir -p $(INSTALL_DIR)
	install -m 755 target/release/$(BINARY_NAME) $(INSTALL_DIR)/$(BINARY_NAME)
	@echo "Installed $(BINARY_NAME) to $(INSTALL_DIR)"
