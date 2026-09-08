# naim-client

Minimal Rust client for controlling a Naim device through its TCP interface and the NVM command protocol.

The client:

* opens a TCP connection to the device;
* performs the Naim handshake;
* automatically reconnects when the connection is lost;
* sends NVM commands and displays the responses received;
* periodically sends a heartbeat.

## Prerequisites

* A recent version of Rust and Cargo;
* a Naim device accessible on the local network;
* the device's IP address and TCP port.

## Configuration

The binary reads `config.toml` from the current working directory:

```toml
device_ip = "192.168.1.43"
port = 15555
timeout = 10
ping_interval = 5
reconnect = 3
```

The fields are as follows:

* `device_ip`: IP address of the Naim device;
* `port`: TCP port used by the protocol;
* `timeout`: heartbeat timeout negotiated during the handshake, in seconds;
* `ping_interval`: interval between two `Ping` commands, in seconds;
* `reconnect`: delay before attempting to reconnect, in seconds.

The configuration can also be overridden using environment variables prefixed with `APP__`, for example:

```sh
APP__DEVICE_IP=192.168.1.50 APP__PORT=15555 cargo run
```

`Config::init()` must be called exactly once before `Config::global()` or before starting the client loops.

## Usage as an Executable

From the project root:

```sh
cargo run
```

The program then waits for one NVM command per line, without `*` or a carriage return:

```text
NVM GETPREAMP
NVM SETRVOL 15
NVM SETINPUT DIGITAL4
quit
```

The client automatically adds the protocol encapsulation, the initial `*`, and the final carriage return. Use `quit` or `exit` to stop input.

## Usage as a Rust Module

The crate exposes a reusable library. In another project, add the local dependency:

```toml
[dependencies]
naim_client = { path = "../naim-client" }
```

Minimal example:

```rust
use naim_client::{connection_manager, heartbeat_loop, Config, SharedConn};
use std::sync::Arc;
use std::thread;

fn main() -> anyhow::Result<()> {
    Config::init()?;

    let config = Config::global().clone();
    let host = format!("{}:{}", config.device_ip, config.port);
    let reconnect = config.reconnect;
    let ping_interval = config.ping_interval;
    let shared = Arc::new(SharedConn::new(host));

    {
        let shared = Arc::clone(&shared);
        thread::spawn(move || connection_manager(shared, reconnect));
    }
    {
        let shared = Arc::clone(&shared);
        thread::spawn(move || heartbeat_loop(shared, ping_interval));
    }

    shared.send_nvm("NVM GETPREAMP")?;
    Ok(())
}
```

This example assumes that the calling project also has a `config.toml` file in its current working directory. `connection_manager` and `heartbeat_loop` are blocking loops intended to run in dedicated threads.

Main API:

* `Config::init()` and `Config::global()`: load and access the global configuration;
* `SharedConn::new(host)`: create the shared connection state;
* `SharedConn::send_nvm(command)`: send an NVM command;
* `SharedConn::send_raw(xml)`: send a raw XML command;
* `SharedConn::send_ping()`: manually send a heartbeat;
* `connection_manager(...)`: connection, handshake, reading, and reconnection;
* `heartbeat_loop(...)`: periodically send `Ping` commands.

NVM responses and errors are currently logged to standard output. The library does not yet provide a callback or Rust channel for receiving application-level events.

## Verification

```sh
cargo check
cargo test
```

The project does not yet contain automated tests; however, `cargo test` still verifies that the targets compile successfully.
