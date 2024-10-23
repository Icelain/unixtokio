use std::env::args;
use std::path::Path;
use std::process::exit;
use tokio::io::{self, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::signal;
use tokio::sync::mpsc::{channel, Receiver};

async fn server(socket_path: String, mut shutdown_receiver: Receiver<()>) {
    let socket_path_buf = Path::new(&socket_path).to_path_buf();
    let socket_path_buf_clone = socket_path_buf.clone();

    let listener = UnixListener::bind(socket_path_buf).expect("Could not create unix socket");

    tokio::spawn(async move {
        match shutdown_receiver.recv().await {
            Some(()) => {
                tokio::fs::remove_file(socket_path_buf_clone)
                    .await
                    .expect("Failed to remove socket file");

                exit(1);
            }
            None => {
                eprintln!(
                    "received nothing from the shutdown receiver. This should not be possible"
                )
            }
        }
    });

    println!("Listening on {socket_path}");
    let mut buffer: [u8; 1024] = [0u8; 1024];
    while let Ok((mut stream, _)) = listener.accept().await {
        tokio::spawn(async move {
            loop {
                match stream.read(&mut buffer).await {
                    Ok(n) => {
                        if n == 0 {
                            break;
                        }

                        println!("client: {:?}", String::from_utf8_lossy(&buffer[..n]));
                    }

                    Err(e) => {
                        eprintln!("Error writing to client; error: {}", e);
                        break;
                    }
                }
            }
        });
    }
}

async fn client(socket_path: String, mut shutdown_receiver: Receiver<()>) {
    let mut unixstream = UnixStream::connect(Path::new(&socket_path)).await.expect("Could not connect to the socket path. Ensure that the path is correct and is being listened on.");

    println!("Connected to {socket_path}");

    tokio::spawn(async move {
        match shutdown_receiver.recv().await {
            Some(()) => {
                println!("Shutting down the client");
                exit(1);
            }
            None => {
                eprintln!(
                    "received nothing from the shutdown receiver. This should not be possible"
                )
            }
        }
    });

    let mut stdout = io::stdout();
    let mut stdin_lines = BufReader::new(io::stdin()).lines();

    loop {
        stdout.write(b"Text: ").await.unwrap();
        stdout.flush().await.unwrap();

        if let Some(line) = stdin_lines.next_line().await.unwrap() {
            unixstream.write(line.as_bytes()).await.unwrap();
        }
    }
}

#[tokio::main]
async fn main() {
    let mode = args().nth(1).unwrap();
    let socket_path = args().nth(2).unwrap();
    let (shutdown_sender, shutdown_receiver) = channel(1);

    tokio::spawn(async move {
        match signal::ctrl_c().await {
            Ok(()) => {
                shutdown_sender.send(()).await.unwrap();
            }
            Err(e) => {
                eprintln!("{}", e)
            }
        }
    });

    if mode.as_str() == "server" {
        server(socket_path, shutdown_receiver).await;
    } else if mode.as_str() == "client" {
        client(socket_path, shutdown_receiver).await;
    } else {
        println!("Provide valid operation");
    }
}
