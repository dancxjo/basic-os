use std::env;
use std::process::Command;
use std::sync::Arc;
use thing_abi::{AbiRequest, ThingRuntime};
use thing_host::HostRuntime;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let mut launch_app = None;

    let mut i = 1;
    while i < args.len() {
        if args[i] == "--launch-app" {
            if i + 1 < args.len() {
                launch_app = Some(args[i + 1].clone());
                i += 1;
            }
        }
        i += 1;
    }

    // Run HostRuntime initialization in a blocking task to avoid "runtime within runtime" panic
    // if HostRuntime::new() uses block_on.
    // Actually, HostRuntime::new() creates its OWN runtime.
    // So we should spawn it in a thread that is NOT a tokio thread?
    // Or just use spawn_blocking?

    let runtime = tokio::task::spawn_blocking(|| Arc::new(HostRuntime::new())).await?;

    // Create socket
    let socket_path = "/tmp/thingos.sock";
    if std::fs::metadata(socket_path).is_ok() {
        std::fs::remove_file(socket_path)?;
    }
    let listener = UnixListener::bind(socket_path)?;
    println!("ThingHost listening on {}", socket_path);

    if let Some(app_name) = launch_app {
        println!("Launching app: {}", app_name);

        tokio::spawn(async move {
            // Wait a bit for socket to be ready
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            let status = Command::new(&app_name)
                .env("THINGOS_SOCKET", socket_path)
                .spawn();

            match status {
                Ok(mut child) => {
                    println!("App {} started", app_name);
                    let _ = child.wait();
                }
                Err(e) => {
                    eprintln!("Failed to launch app {}: {}", app_name, e);
                }
            }
        });
    }

    loop {
        let (mut socket, _) = listener.accept().await?;
        let runtime = runtime.clone();

        tokio::spawn(async move {
            let mut buf = vec![0u8; 65536]; // Buffer for requests

            loop {
                // Read length
                let mut len_buf = [0u8; 4];
                if socket.read_exact(&mut len_buf).await.is_err() {
                    break;
                }
                let len = u32::from_le_bytes(len_buf) as usize;

                if len > buf.len() {
                    eprintln!("Request too large: {}", len);
                    break;
                }

                if socket.read_exact(&mut buf[..len]).await.is_err() {
                    break;
                }

                let req: AbiRequest = match postcard::from_bytes(&buf[..len]) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("Failed to deserialize request: {}", e);
                        break;
                    }
                };

                let resp = runtime.call(req);

                let resp_bytes = match postcard::to_allocvec(&resp) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("Failed to serialize response: {}", e);
                        break;
                    }
                };

                let resp_len = (resp_bytes.len() as u32).to_le_bytes();
                if socket.write_all(&resp_len).await.is_err() {
                    break;
                }
                if socket.write_all(&resp_bytes).await.is_err() {
                    break;
                }
            }
        });
    }
}
