WEECHAT_HOME ?= $(HOME)/.weechat
PREFIX ?= $(WEECHAT_HOME)

.PHONY: install install-dir lint

target/debug/libgo.so: src/lib.rs
	cargo build

install: install-dir target/debug/libgo.so
	install -m644  target/debug/libgo.so $(DESTDIR)$(PREFIX)/plugins/go.so

install-dir:
	install -d $(DESTDIR)$(PREFIX)/plugins

lint:
	cargo clippy
