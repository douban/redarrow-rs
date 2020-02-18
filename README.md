# redarrow-rs

Execute commands on remote servers.

[![crates.io](https://img.shields.io/crates/d/redarrow.svg)](https://crates.io/crates/redarrow)
![License](https://img.shields.io/crates/l/redarrow.svg)

## client example

```rust
let client = webclient::Client::new(host, 4205, command, arguments);
let result = client.run_command();
```

## run server

```shell
export RUST_LOG="actix_web=info"
redarrow-server -c misc/example.conf
```

## run client

```shell
redarrow-client uptime
```
