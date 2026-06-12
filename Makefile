.PHONY: all build test test-cargo test-gcc test-llvm clean test-build-apps test-build-sqlite test-build-doom test-build-lua test-build-lisp

all: build

build:
	cargo build --workspace

build-static-linux:
	mkdir -p dist
	docker build --build-arg HOST_OS=$$(uname -s) --target exporter --output type=local,dest=dist -f Dockerfile.build.linux .

build-static-macos:
	mkdir -p dist
	docker build --target exporter --output type=local,dest=dist -f Dockerfile.build.macos .

build-macos:
	cargo build --release --workspace
	mkdir -p dist
	cp target/release/stricc dist/stricc-macos

build-all: build-static-linux build-macos

test: test-cargo test-gcc test-llvm

test-cargo: build
	cargo test --workspace

test-gcc: build
	python3 stricc/tests/gcc_torture_runner.py

test-llvm: build
	python3 stricc/tests/llvm_test_suite_runner.py

test-build-apps: test-build-sqlite test-build-doom test-build-lua test-build-lisp

test-build-sqlite: build
	mkdir -p target/apps
	curl -s -O https://www.sqlite.org/2024/sqlite-amalgamation-3460000.zip
	unzip -o sqlite-amalgamation-3460000.zip -d target/apps
	rm sqlite-amalgamation-3460000.zip
	./target/debug/stricc -O2 -c target/apps/sqlite-amalgamation-3460000/sqlite3.c -o target/apps/sqlite3.o
	./target/debug/stricc -O2 -c target/apps/sqlite-amalgamation-3460000/shell.c -o target/apps/shell.o
	clang -O2 target/apps/shell.o target/apps/sqlite3.o -lpthread -ldl -lm -o target/apps/sqlite3

test-build-doom: build
	mkdir -p target/apps
	if [ ! -d target/apps/doomgeneric ]; then \
		git clone --depth 1 https://github.com/ozkl/doomgeneric.git target/apps/doomgeneric; \
	fi
	make -C target/apps/doomgeneric/doomgeneric -f Makefile.sdl CC=$$PWD/target/debug/stricc || true
	make -C target/apps/doomgeneric/doomgeneric -f Makefile CC=$$PWD/target/debug/stricc || true

test-build-lua: build
	mkdir -p target/apps
	if [ ! -d target/apps/lua ]; then \
		git clone --depth 1 https://github.com/lua/lua.git target/apps/lua; \
	fi
	make -C target/apps/lua generic CC=$$PWD/target/debug/stricc || true

test-build-lisp: build
	mkdir -p target/apps
	if [ ! -d target/apps/minilisp ]; then \
		git clone --depth 1 https://github.com/rui314/minilisp.git target/apps/minilisp; \
	fi
	./target/debug/stricc -O2 target/apps/minilisp/minilisp.c -o target/apps/minilisp_bin || true

clean:
	cargo clean
	rm -rf target/gcc-torture-temp target/llvm-test-suite-temp target/apps
