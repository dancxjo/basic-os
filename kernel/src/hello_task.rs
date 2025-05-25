use wasmi::{Config, Engine, Instance, Linker, Module, Store};

use crate::{bootloader::get_module, scheduler::WasmTask, serial_print};

pub fn create_hello_task(pid: usize) -> anyhow::Result<WasmTask> {
    let wasm_bytes = get_module("/boot/hello_from.wasm")
        .ok_or_else(|| anyhow::anyhow!("Failed to load hello_from.wasm"))?;

    // Create a Wasm engine and module
    let mut config = Config::default();
    config.consume_fuel(true); // Enable fuel metering
    let engine = Engine::new(&config);
    let mut store = Store::new(&engine, ());
    let module = Module::new(&engine, &wasm_bytes)?;
    let mut linker = Linker::new(&engine);

    linker.func_wrap("env", "putchar", |_: wasmi::Caller<'_, ()>, ch: i32| {
        serial_print!("{}", ch as u8 as char);
    })?;

    linker.func_wrap(
        "env",
        "get_pid",
        move |_caller: wasmi::Caller<'_, ()>| -> i32 { pid as i32 },
    )?;

    let instance = linker.instantiate(&mut store, &module)?.start(&mut store)?;

    Ok(WasmTask {
        pid: pid,
        store,
        instance,
        state: crate::scheduler::TaskState::Runnable,
    })
}
