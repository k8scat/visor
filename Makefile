NAME = visor
VERSION = 0.1.15

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

sync:
	rsync -v --progress target/release/visor ones-priv:/usr/bin/visor
	rsync -v --progress target/release/visor-serv ones-priv:/usr/bin/visor-serv
