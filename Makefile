NAME = visor
VERSION = 0.1.0

.PHONY: build
build:
	cargo build

.PHONY: release
release:
	cargo build --release
