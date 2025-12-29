BINARY ?= calculator_cli
PREFIX ?= /usr/local
DESTDIR ?=

.PHONY: build release install uninstall clean

build:
	cargo build

release:
	cargo build --release

install: release
	install -Dm755 target/release/$(BINARY) $(DESTDIR)$(PREFIX)/bin/$(BINARY)

uninstall:
	rm -f $(DESTDIR)$(PREFIX)/bin/$(BINARY)

clean:
	cargo clean
