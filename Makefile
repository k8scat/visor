NAME = visor
VERSION = 0.1.5

.PHONY: build
build:
	cargo build

.PHONY: build-visor
build-visor:
	cargo build -p visor

.PHONY: release
release:
	cargo build --release
	gh release create v$(VERSION) \
		--generate-notes \
		target/release/visor target/release/visor-serv
