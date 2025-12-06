# Architecture Overview

## Bundles, Instances, and Processes

In ThingOS, a **Bundle** is an installable unit of code plus metadata. It is not the same thing as a process.

A **Bundle** describes:
*   What kind of thing it is (app, driver, widget, service, etc.).
*   Which ABI it implements.
*   How it should be wired into the system (capabilities, devices, graph roles).

At runtime, the system creates **instances** of a bundle:
*   Each instance has its own identity (task ID, graph node, state).
*   Multiple instances of the same bundle can run at once (e.g., two terminals, many widgets).
*   Instances may be scheduled and isolated in different ways (threads, wasm isolates, processes) depending on policy.

A **Process** (or protection domain) is just one possible container for instances:
*   One process may host many lightweight instances.
*   Or a single instance may get its own process for isolation.
*   But this is a deployment decision, not a property of the bundle itself.

In other words:
*   **"Bundle"** = code + contract.
*   **"Instance"** = a live actor created from that bundle.
*   **"Process"** = one possible sandbox for running instances.

Each kind of bundle uses a specific ABI to talk to the rest of the system. The ABI is chosen by its bundle kind, not by its process model.

### Bundle Kinds + ABIs

The ABI is tied to the bundle’s kind, not to "is this its own process".

#### App Bundle
*   **Kind**: `app`
*   **ABI**: `AppABI`
*   **Entry**: `fn main(ctx: AppContext)`
*   **Capabilities**: Window handle(s), access to graph, syscalls, stdin/stdout-style channels.

#### Driver Bundle
*   **Kind**: `driver`
*   **ABI**: `DriverABI`
*   **Entry**: `fn driver_main(ctx: DriverContext)`
*   **Capabilities**: IRQ/event channels, access to device registers / MMIO, exposes a device node in `/dev` and/or graph endpoints.

#### Service/Daemon Bundle
*   **Kind**: `service`
*   **ABI**: `ServiceABI`
*   **Entry**: `fn service_main(ctx: ServiceContext)`
*   **Capabilities**: RPC endpoints, timers, graph subscriptions.

#### Widget Bundle
*   **Kind**: `widget` (or module loaded by widgetd)
*   **ABI**: `WidgetABI`
*   **Functions**:
    *   `fn init(ctx: WidgetCtx) -> State`
    *   `fn draw(state: &State, fb: &mut Framebuffer, rect: Rect)`
    *   `fn handle_event(state: &mut State, event: WidgetEvent)`
    *   `fn teardown(State)`
*   **Note**: Instances of this ABI are WidgetIsolates; they can be hosted many-per-process or one-per-process depending on isolation strictness.

### Unifying Principle

You can have:
*   A bunch of app instances sharing one userland process (hosted mode), all using `AppABI`.
*   The exact same bundles running one-instance-per-process on bare metal for stronger isolation.
*   Widgets all using `WidgetABI`, regardless of whether their isolates are separate processes or Wasm instances inside `widgetd`.

## Environment Abstraction

Core environment services are being pulled behind lightweight traits in `thing_model::env`:
*   `Log` handles `info`, `warn`, and `error` messages.
*   `Clock` reports a monotonic timestamp as a `u64` (std hosts currently use microseconds since the env was created).

Std-backed helpers live in `thingos_kernel_std` (`StdLog`, `StdClock`, and `StdEnv` via the prelude) so host tools can swap `println!` and ad-hoc timers for structured hooks. `thing_host::HostRuntime` uses `StdEnv` to announce which graph backend is active as the first client of this abstraction.

## System Modes

ThingOS supports distinct system modes that define the top-level UI and interaction model. Modes are managed by the Compositor and can be switched using global keybindings.

### Core Modes

1.  **Sky Mode** (`Mode::Sky`)
    *   The default desktop environment.
    *   Supports multiple overlapping windows, a desktop background (clouds), and a visible graph viewer.
    *   Standard window management (move, resize, minimize).
    *   Activated via `F1`.

2.  **Max Mode** (`Mode::Max`)
    *   A single-tasking, distraction-free environment.
    *   The currently active program is forced to fullscreen, covering the entire display.
    *   No window decorations (title bars, borders) or background are visible.
    *   Switching active programs automatically maximizes the new program.
    *   Activated via `F2`.

### Mode Switching

*   **F1**: Switch to Sky Mode.
*   **F2**: Switch to Max Mode.
*   **F12**: "Return to Active Program".
    *   If a program is active, switches to Max Mode and ensures it is focused and visible.
    *   If no program is active, defaults to Sky Mode.

### Implementation Details

The `Compositor` maintains the `active_mode` state.
*   When entering **Max Mode**, the active window's current geometry is saved, and it is resized to fill the screen.
*   When leaving **Max Mode** (returning to Sky), the window's original geometry is restored.
*   The Compositor's rendering loop adapts to the mode:
    *   In Sky Mode, it draws the background and all windows with decorations.
    *   In Max Mode, it draws only the active window (frameless) or the background if no window is active.
