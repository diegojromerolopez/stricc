.PHONY: all build test test-cargo test-gcc test-llvm clean

all: build

build:
	cargo build --workspace

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
