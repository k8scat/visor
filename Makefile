NAME = visor
VERSION = 0.1.2

.PHONY: build
build:
	cargo build

.PHONY: release
release:
	cargo build --release
	gh release create v$(VERSION) target/release/visor target/release/visor-serv
