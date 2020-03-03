[![Stable Test](https://github.com/Hexilee/roa/workflows/Stable%20Test/badge.svg)](https://github.com/Hexilee/roa/actions)
[![codecov](https://codecov.io/gh/Hexilee/roa/branch/master/graph/badge.svg)](https://codecov.io/gh/Hexilee/roa)
[![Rust Docs](https://docs.rs/roa-tls/badge.svg)](https://docs.rs/roa-tls)
[![Crate version](https://img.shields.io/crates/v/roa-tls.svg)](https://crates.io/crates/roa-tls)
[![Download](https://img.shields.io/crates/d/roa-tls.svg)](https://crates.io/crates/roa-tls)
[![Version](https://img.shields.io/badge/rustc-1.40+-lightgray.svg)](https://blog.rust-lang.org/2019/12/19/Rust-1.40.0.html)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/Hexilee/roa/blob/master/LICENSE)

## Roa-tls

This crate provides an acceptor implementing `roa_core::Accept` and an app extension.

### TlsIncoming

```rust
use roa_core::App;
use roa_tls::TlsIncoming;
use roa_tls::rustls::{ServerConfig, NoClientAuth};
use roa_tls::rustls::internal::pemfile::{certs, rsa_private_keys};
use std::fs::File;
use std::io::BufReader;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut config = ServerConfig::new(NoClientAuth::new());
let mut cert_file = BufReader::new(File::open("../assets/cert.pem")?);
let mut key_file = BufReader::new(File::open("../assets/key.pem")?);
let cert_chain = certs(&mut cert_file).unwrap();
let mut keys = rsa_private_keys(&mut key_file).unwrap();
config.set_single_cert(cert_chain, keys.remove(0))?;

let incoming = TlsIncoming::bind("127.0.0.1:0", config)?;
let server = App::new(()).accept(incoming);
// server.await
Ok(())
# }
```

### TlsListener

```rust
use roa_core::App;
use roa_tls::TlsListener;
use roa_tls::rustls::{ServerConfig, NoClientAuth};
use roa_tls::rustls::internal::pemfile::{certs, rsa_private_keys};
use std::fs::File;
use std::io::BufReader;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut config = ServerConfig::new(NoClientAuth::new());
let mut cert_file = BufReader::new(File::open("../assets/cert.pem")?);
let mut key_file = BufReader::new(File::open("../assets/key.pem")?);
let cert_chain = certs(&mut cert_file).unwrap();
let mut keys = rsa_private_keys(&mut key_file).unwrap();
config.set_single_cert(cert_chain, keys.remove(0))?;

let (addr, server) = App::new(()).listen_tls_on("127.0.0.1:0", config)?;
// server.await
Ok(())
# }
```
