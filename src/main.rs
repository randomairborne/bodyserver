use std::{ffi::OsString, net::Ipv4Addr, time::Duration};

use axum::{body::Body, extract::State, routing::get};
use cyclingbody::{CyclingBody, CyclingBodySource};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let folder = std::env::args()
        .nth(1)
        .ok_or("This application requires an argument for the path to the frame-files")?;

    let mut frames: Vec<(OsString, Vec<u8>)> = Vec::new();

    let dir = match std::fs::read_dir(&folder) {
        Ok(v) => v,
        Err(e) => {
            return Err(format!("Error reading directory {folder}: {e}").into());
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

    let bodies = CyclingBodySource::from_vecs(frames, Duration::from_secs_f64(1. / 30.))?;
    let app = axum::Router::new()
        .route("/", get(cycle))
        .with_state(bodies);

    let tcp = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 8080)).await?;
    axum::serve(tcp, app).await?;
    Ok(())
}

async fn cycle(State(b): State<CyclingBodySource>) -> Body {
    axum::body::Body::from_stream(CyclingBody::from(b))
}
