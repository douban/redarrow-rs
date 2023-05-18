.PHONY: run_test_server

build:
	cargo build --verbose

run_test_server: build
	RUST_LOG=debug ./target/debug/redarrow-server -c ./misc/example.conf -p 4206
