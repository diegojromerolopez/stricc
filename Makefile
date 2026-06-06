.PHONY: all build test test-cargo test-gcc test-llvm clean

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
	cp target/release/libstricc_rt.a dist/libstricc_rt-macos.a

build-all: build-static-linux build-macos

test: test-cargo test-gcc test-llvm

test-cargo: build
	cargo test --workspace

test-gcc: build
	python3 stricc/tests/gcc_torture_runner.py

test-llvm: build
	python3 stricc/tests/llvm_test_suite_runner.py

clean:
	cargo clean
	rm -rf target/gcc-torture-temp target/llvm-test-suite-temp
