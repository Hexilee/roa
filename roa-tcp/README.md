[![Stable Test](https://github.com/Hexilee/roa/workflows/Stable%20Test/badge.svg)](https://github.com/Hexilee/roa/actions)
[![codecov](https://codecov.io/gh/Hexilee/roa/branch/master/graph/badge.svg)](https://codecov.io/gh/Hexilee/roa)
[![Rust Docs](https://docs.rs/roa-tcp/badge.svg)](https://docs.rs/roa-tcp)
[![Crate version](https://img.shields.io/crates/v/roa-tcp.svg)](https://crates.io/crates/roa-tcp)
[![Download](https://img.shields.io/crates/d/roa-tcp.svg)](https://crates.io/crates/roa-tcp)
[![Version](https://img.shields.io/badge/rustc-1.40+-lightgray.svg)](https://blog.rust-lang.org/2019/12/19/Rust-1.40.0.html)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/Hexilee/roa/blob/master/LICENSE)

## Roa-tcp

This crate provides an acceptor implementing `roa_core::Accept` and an app extension.

### TcpIncoming

```
use roa_core::App;
use roa_tcp::TcpIncoming;
use std::io;

# fn main() -> io::Result<()> {
let app = App::new(());
let incoming = TcpIncoming::bind("127.0.0.1:0")?;
let server = app.accept(incoming);
// server.await
Ok(())
# }
```

### Listener

```
use roa_core::App;
use roa_tcp::Listener;
use std::io;

# fn main() -> io::Result<()> {
let app = App::new(());
let (addr, server) = app.listen_on("127.0.0.1:0")?;
// server.await
Ok(())
# }
```