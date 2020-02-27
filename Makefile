.PHONY: run_test_server


run_test_server:
	RUST_LOG=debug ./target/debug/redarrow-server -c ./misc/example.conf -p 4206
