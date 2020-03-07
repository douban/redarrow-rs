# redarrow-rs

Execute commands on remote servers.

![build](https://github.com/douban/redarrow-rs/workflows/test/badge.svg)
[![crates.io](https://img.shields.io/crates/v/redarrow.svg)](https://crates.io/crates/redarrow)
![License](https://img.shields.io/crates/l/redarrow.svg)

## client example

```rust
let client = webclient::Client::new(host, 4205, command, arguments);
let result = client.run_command().await;
```

## run server

```shell
export RUST_LOG="info"
redarrow-server -c misc/example.conf
```

## run client

```shell
redarrow-client uptime
```
