# Crate Documentation

**Version:** 0.1.0

**Format Version:** 39

# Module `jpe`

Remote control of the JPE CPSC1 controller and associated modules.

The `jpe` crate provides a Rust and Python implementation, via `PyO3`,
for controlling and administering the CPSC1 controller and the following
modules:
* RSM
* CADM2

Please view [API documentation](https://www.jpe-innovations.com/wp-content/uploads/CNP_MAN02_R05_Software-User-Manual.pdf)
provided by JPE for more details.

Only commands supported by the previously mentioned modules are implemented, PRs are welcome
for adding support for other modules!

# Example
This example opens a connection to the controller using serial transport
and queries for the supported cryo stage SKUs.

```rust
use jpe::BaseContextBuilder;

// On Windows, use something like "COM1" or "COM15".
let mut ctx = BaseContextBuilder::new().with_serial("/dev/cu.usbserial-D30IYJT2").build()?;
let supported_stages = ctx.get_supported_stages()?;
```
# Example
This example opens a connection to the controller using network transport and
enables scan mode (E.g. for driving a piezo scanner) on the CADM2 module in slot one
of the controller cabinet.

```rust
use jpe::{BaseContextBuilder, Slot};

let mut ctx = BaseContextBuilder::new().with_network("169.254.10.10").build()?;
let _ = ctx.enable_scan_mode(Slot::One, 512)?;
```