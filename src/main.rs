mod server;

pub mod pb {
    tonic::include_proto!("grpc.examples.echo");
}

const ADDR: &'static str = "127.0.0.1:8080";

use metrics_runtime::{exporters::LogExporter, observers::YamlBuilder, Receiver};
use std::time::{Duration, Instant};

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init_custom_env("info");

    let recv = Receiver::builder()
        .build()
        .expect("failed to create receiver");

    let mut exporter = LogExporter::new(
        recv.controller(),
        YamlBuilder::new(),
        log::Level::Info,
        Duration::from_secs(1),
    );

    // std::thread::spawn(move || exporter.run());

    recv.install();

    let rt1 = tokio::runtime::Builder::new()
        .enable_all()
        .core_threads(8)
        .threaded_scheduler()
        .build()
        .unwrap();

    let mut rt2 = tokio::runtime::Builder::new()
        .enable_all()
        .core_threads(8)
        .threaded_scheduler()
        .build()
        .unwrap();

    let addr = ADDR.parse()?;
    rt1.spawn(server::run(addr));

    println!("Sending requests...");

    rt2.block_on(async move {
        run_client().await.unwrap();
    });

    println!("done sending requests");

    exporter.turn();

    Ok(())
}

async fn run_client() -> anyhow::Result<()> {
    let conn = tonic::transport::Channel::from_shared(format!("http://{}", ADDR))?
        .connect()
        .await?;

    let mut client = pb::echo_client::EchoClient::new(conn);

    for _ in 0..10_000 {
        let request = tonic::Request::new(pb::EchoRequest {
            message: "hello".into(),
        });

        let start = Instant::now();
        client.unary_echo(request).await?;
        let end = Instant::now();

        metrics::timing!("request_duration", start, end);
    }

    Ok(())
}
