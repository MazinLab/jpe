
# `jpe`

Library enabling remote control of the JPE CPSC1 controller and associated modules over network and serial
transport.

The `jpe` crate provides a cross-platform, command-level driver for controlling and administering the [Cryo Positioning Systems Controller (CPSC1)](https://www.jpe-innovations.com/cryo-uhv-products/cryo-positioning-systems-controller/), which is required to actuate various positioning stages available from [jpe](https://www.jpe-innovations.com/).
Both Python and Rust applications are supported.

Currently, only the following function specific modules are supported:
* RSM
* CADM2

Please view the [API documentation](https://www.jpe-innovations.com/wp-content/uploads/CNP_MAN02_R05_Software-User-Manual.pdf)
provided by JPE for more details.

Only commands supported by the previously mentioned modules are implemented, PRs are welcome
for adding support for other modules!

# Rust Example
This example opens a connection to the controller using serial transport
and queries for the supported cryo stage SKUs.

```rust
use jpe::BaseContextBuilder;

// On Windows, use something like "COM1" or "COM15".
let mut ctx = BaseContextBuilder::new().with_serial("/dev/cu.usbserial-D30IYJT2").build()?;
let supported_stages = ctx.get_supported_stages()?;
```
This example opens a connection to the controller using network transport and
enables scan mode (E.g. for driving a piezo scanner) on the CADM2 module in slot one
of the controller cabinet.

```rust
use jpe::{BaseContextBuilder, Slot};

let mut ctx = BaseContextBuilder::new().with_network("169.254.10.10").build()?;
let _ = ctx.enable_scan_mode(Slot::One, 512)?;
```

 # Using Python
 To compile Python bindings and install as a module in the active virtual environment, the
 Python package [`maturin`](https://www.maturin.rs/) should be installed and used.
 After cloning the `jpe` repo, run the following shell command from the crate root
 (be sure to activate the appropriate virtual env):
```
 maturin develop --features python
```

 The module should now be installed and can be used with the Python ecosystem. To help with type hints
 and docstrings in modern IDEs, an optional wrapper module, [`jpe_python`](https://github.com/MazinLab/jpe_python),
 can be used. Using this wrapper, the construction of the Controller context is more pythonic. If Rust builder ergonomics are
  desired, one can forego the convenience given by the wrapper and use the FFI directly.

 ## Using the FFI directly
 ```python
 from jpe_python_ffi import BaseContextBuilder, Slot

 ctx = BaseContextBuilder().with_network("169.254.10.10").build()
 ctx.enable_scan_mode(Slot.one(), 512)
 ```

 ## Using the `jpe_python` wrapper module.
 Note the difference in syntax for the constructor and the enums passed as arguments.
 ```python
 from jpe_python import ControllerContext, ModuleChannel, Slot

 ctx = ControllerContext.with_serial("/dev/cu.usbserial-D30IYJT2")
 ctx.set_neg_end_stop(Slot().four, ModuleChannel().one)
 ```
