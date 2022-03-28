NAME = visor
VERSION = 0.1.5

.PHONY: build
build:
	cargo build

.PHONY: release
release:
	cargo build --release
	gh release create v$(VERSION) \
		--generate-notes \
		target/release/visor target/release/visor-serv
