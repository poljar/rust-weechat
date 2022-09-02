WEECHAT_HOME ?= $(HOME)/.weechat
PREFIX ?= $(WEECHAT_HOME)

.PHONY: install install-dir lint

target/debug/libinfolist.so: src/lib.rs
	cargo build

install: install-dir target/debug/libinfolist.so
	install -m644  target/debug/libinfolist.so $(DESTDIR)$(PREFIX)/plugins/infolist.so

install-dir:
	install -d $(DESTDIR)$(PREFIX)/plugins

lint:
	cargo clippy
