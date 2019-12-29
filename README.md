# redarrow-rust

Execute commands on remote servers.

## example

```rust
let client = webclient::Client::new(host, 4205, command, arguments);
let result = client.run_command();
```

