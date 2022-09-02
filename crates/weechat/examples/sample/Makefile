WEECHAT_HOME ?= $(HOME)/.weechat
PREFIX ?= $(WEECHAT_HOME)

.PHONY: install install-dir lint

target/debug/libgo.so: src/lib.rs
	cargo build

install: install-dir target/debug/librust.so
	install -m644  target/debug/librust.so $(DESTDIR)$(PREFIX)/plugins/rust.so

install-dir:
	install -d $(DESTDIR)$(PREFIX)/plugins

lint:
	cargo clippy
