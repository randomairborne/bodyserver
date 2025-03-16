use std::{convert::Infallible, ffi::OsString, net::Ipv4Addr, time::Duration};

use cyclingbody::{CyclingBody, CyclingBodySource};
use hyper::{Response, server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

#[derive(argh::FromArgs)]
#[argh(description = "Play ascii conversions of video files over cURL")]
struct Args {
    #[argh(positional, description = "folder to read frames out of")]
    folder: String,
    #[argh(option, short = 'f', default = "24.0", description = "your desired framerate, in frames per second")]
    framerate: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args: Args =  argh::from_env();

    let mut frames: Vec<(OsString, Vec<u8>)> = Vec::new();

    let dir = match std::fs::read_dir(&args.folder) {
        Ok(v) => v,
        Err(e) => {
            return Err(format!("Error reading directory `{}`: {e}", args.folder).into());
        }
    };

    for file in dir {
        let Ok(file) = file else {
            continue;
        };

        if file.file_type()?.is_file() {
            let mut data = match std::fs::read(file.path()) {
                Ok(v) => v,
                Err(e) => {
                    let fname = file.file_name();
                    let name = fname.to_string_lossy();
                    return Err(format!("Error reading file {name}: {e}").into());
                }
            };

            // Always add a \n and a clear to the file
            data.extend(b"\n\x1b[2J");
            frames.push((file.file_name(), data));
        }
    }

    frames.sort_by(|a, b| a.0.cmp(&b.0));
    let frames = frames.into_iter().map(|v| v.1).collect();

    let bodies = CyclingBodySource::from_vecs(frames, Duration::from_secs_f64(1. / args.framerate))?;

    let tcp = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 8080)).await?;
    serve(tcp, bodies).await?;
    Ok(())
}

async fn serve(
    listener: TcpListener,
    source: CyclingBodySource,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // We start a loop to continuously accept incoming connections
    loop {
        let (stream, _) = listener.accept().await?;

        // Use an adapter to access something implementing `tokio::io` traits as if they implement
        // `hyper::rt` IO traits.
        let io = TokioIo::new(stream);
        let source = source.clone();

        // Spawn a tokio task to serve multiple connections concurrently
        tokio::task::spawn(async move {
            let source = source.clone();
            // Finally, we bind the incoming connection to our `hello` service
            if let Err(err) = http1::Builder::new()
                // `service_fn` converts our function in a `Service`
                .serve_connection(
                    io,
                    service_fn(move |_| {
                        let source = source.clone();
                        let body = CyclingBody::from(source);
                        async move { Ok::<_, Infallible>(Response::new(body)) }
                    }),
                )
                .await
            {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}
