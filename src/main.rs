use anyhow::{Context, Result};
use clap::Parser;
use futures::stream::{FuturesUnordered, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Semaphore;

/// Fast concurrent uploader for local files
/// Only runs if the destination host is localhost
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Fast concurrent uploader for local files to a localhost endpoint"
)]
struct Args {
    /// Directory containing files to upload (files may have no extension)
    #[arg(short, long)]
    dir: PathBuf,

    /// Destination endpoint (must be localhost, e.g. http://localhost:8080/api/v2/sbom)
    #[arg(short, long)]
    url: String,

    /// Number of concurrent requests (workers). Higher = faster, but heavier on CPU/network.
    #[arg(short = 'c', long, default_value_t = 32)]
    concurrency: usize,

    /// Number of files processed per iteration batch
    #[arg(short = 'b', long, default_value_t = 128)]
    batch_size: usize,

    /// Request timeout seconds
    #[arg(short = 't', long, default_value_t = 300)]
    timeout_s: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load all files (any file, no extension filter)
    let mut entries: Vec<PathBuf> = fs::read_dir(&args.dir)
        .with_context(|| format!("reading directory {:?}", &args.dir))?
        .filter_map(|res| res.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();

    entries.sort();

    let total = entries.len();
    println!("Found {} files in {:?}", total, &args.dir);

    if total == 0 {
        println!("No files to upload. Exiting.");
        return Ok(());
    }

    let client = Client::builder()
        .connect_timeout(Duration::from_secs(args.timeout_s))
        .timeout(Duration::from_secs(args.timeout_s))
        .pool_max_idle_per_host(args.concurrency)
        .build()?;

    let pb = ProgressBar::new(total as u64);

    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] \
         [{wide_bar:.cyan/blue}] {pos}/{len} ({percent}%)  \
         ETA: {eta_precise}",
        )
        .unwrap(),
    );
    // pb.set_style(
    //     ProgressStyle::with_template(
    //         "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}",
    //     )
    //     .unwrap()
    //     .tick_strings(&["-", "\\", "|", "/"]),
    // );

    // concurrency limiter + shared log file
    let semaphore = Arc::new(Semaphore::new(args.concurrency));
    let failures_file = Arc::new(Mutex::new(
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("failures.log")
            .context("opening failures.log")?,
    ));

    // Process in chunks (for memory control)
    for chunk in entries.chunks(args.batch_size) {
        let mut futures = FuturesUnordered::new();

        for p in chunk.iter() {
            let path = p.clone();
            let client = client.clone();
            let url = args.url.clone();
            let permit = semaphore.clone().acquire_owned();
            let failures_file = failures_file.clone();
            let pb = pb.clone();

            // spawn an async task per file
            futures.push(tokio::spawn(async move {
                let _permit = permit.await;
                match tokio::fs::read(&path).await {
                    Ok(bytes) => {
                        let res = client
                            .post(&url)
                            .header("Content-Type", "application/json")
                            .body(bytes)
                            .send()
                            .await;
                        match res {
                            Ok(resp) if resp.status().is_success() => {
                                pb.inc(1);
                                Ok::<(), (PathBuf, String)>(())
                            }
                            Ok(resp) => {
                                let msg = format!(
                                    "HTTP {} for {}",
                                    resp.status().as_u16(),
                                    path.display()
                                );
                                let mut file = failures_file.lock().unwrap();
                                writeln!(file, "{} | {}", path.display(), msg).ok();
                                pb.inc(1);
                                Err((path, msg))
                            }
                            Err(e) => {
                                let msg = format!("ERR {} for {}", e, path.display());
                                let mut file = failures_file.lock().unwrap();
                                writeln!(file, "{} | {}", path.display(), msg).ok();
                                pb.inc(1);
                                Err((path, msg))
                            }
                        }
                    }
                    Err(e) => {
                        let msg = format!("READ_ERR {} for {}", e, path.display());
                        let mut file = failures_file.lock().unwrap();
                        writeln!(file, "{} | {}", path.display(), msg).ok();
                        pb.inc(1);
                        Err((path, msg))
                    }
                }
            }));
        }

        // wait for this batch
        while let Some(join_res) = futures.next().await {
            if let Err(join_err) = join_res {
                eprintln!("Task join error: {join_err}");
            }
        }
    }

    pb.finish_with_message("Done.");
    println!("Finished. Check failures.log for any failed uploads.");
    Ok(())
}
