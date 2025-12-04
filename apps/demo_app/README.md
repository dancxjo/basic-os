# Demo App

This app demonstrates a rich layout with multiple widgets and windows, wired up via the graph.

## Features

- **Top Status Bar**: Shows title and time.
- **Left Launcher Rail**: Vertical list of icons.
- **Main Content**: Placeholder for main app content.
- **Graph Mini Viewer**: A miniature view of the graph.
- **Thing Inspector**: Inspects properties of a selected Thing.
- **Status Widget**: Shows system status.

## How to Run

1. Build the project:
   ```bash
   make
   ```

2. Run in QEMU:
   ```bash
   make run
   ```

The demo app should launch automatically (or can be launched from the shell if configured).
It sets up a dashboard layout with multiple windows.
