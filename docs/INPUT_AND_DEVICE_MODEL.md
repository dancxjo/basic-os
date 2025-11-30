# Input & Device Model: Native vs Hosted

ThingOS now has a unified device ABI that supports two runtime modes while keeping the same userland drivers:

## Native (bare metal)

Userland drivers call into the kernel via real syscalls.

The kernel exposes raw hardware device buffers:

*   PS/2 keyboard bytes via `DevRead(kbd0)`.
*   Mouse packet bytes via `DevRead(mouse0)`.
*   Framebuffer memory via the existing mapping mechanism.

The kernel does no high-level decoding; it just pushes bytes from the PS/2 controller into an `InputBuffer<u8, 256>`.

## Hosted (Linux)

Userland drivers are compiled for `std` and talk to the host via `AbiRequest`.

`thing_host` emulates virtual hardware FIFOs:

*   `keyboard_fifo`: `VecDeque<u8>`
*   `mouse_fifo`: `VecDeque<u8>`

`DevRead` on `kbd0` / `mouse0` pops bytes from these FIFOs, mimicking a serial hardware stream.

A host-side compositor / runner injects bytes (e.g., scancodes) into the FIFOs via an HTTP endpoint.

## ABI Surface

The low-level device ABI is now expressed via `thing_abi::AbiRequest`:

*   `DevOpen` – open a device (keyboard, mouse, framebuffer, etc.)
*   `DevRead` – read raw bytes from a device
*   `DevWrite` – write raw bytes to a device
*   `IrqBind` – bind a driver to an IRQ
*   `IrqAck` – acknowledge an IRQ

On bare metal (`target_os = "none"`), `sys.rs` uses real syscalls.
On hosted builds, `sys.rs` translates those same calls into `AbiRequest`s that are handled by `thing_host`:

```rust
#[cfg(target_os = "none")]
// → real syscall instructions

#[cfg(not(target_os = "none"))]
// → send AbiRequest to host_runtime() / thing_host
```

This lets the same userland driver code run against real hardware (via the kernel) or against virtual hardware (via `thing_host`).

## Injecting Keyboard Input in Hosted Mode

In hosted mode, `thing_host` exposes a simple HTTP endpoint to inject raw keyboard bytes into the virtual keyboard FIFO:

```bash
# Decimal 30 (0x1E) is Set 1 scancode for the 'A' key
curl -X POST -d "30" http://127.0.0.1:8080/input/key
```

This simulates pressing the 'A' key at the virtual hardware level:

1.  The HTTP handler parses "30" as a byte and pushes it into `keyboard_fifo`.
2.  Userland `keyboard_driver` eventually calls `DevRead(kbd0)` (via `sys::dev_read`).
3.  `thing_host` services `DevRead` by popping from `keyboard_fifo` and returning that byte.
4.  The keyboard driver decodes the scancode and forwards the resulting key event into the rest of the system (e.g., compositor / focused window).

For now, this is a minimal test harness. Longer term, a browser frontend or other host UI can send scancodes to `/input/key` (or a more structured socket protocol) to provide real-time keyboard input.
