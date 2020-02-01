mod server;

pub mod pb {
    tonic::include_proto!("grpc.examples.echo");
}

const ADDR: &'static str = "127.0.0.1:8080";

use metrics_runtime::{exporters::LogExporter, observers::YamlBuilder, Receiver};
use quanta::Clock;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    std::env::set_var("RUST_LOG", "info");
    pretty_env_logger::init();

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

    log::info!("Sending requests...");

    rt2.block_on(async move {
        run_client().await.unwrap();
    });

    log::info!("done sending requests");

    exporter.turn();

    Ok(())
}

async fn run_client() -> anyhow::Result<()> {
    let clock = Clock::new();

    let mut j = Vec::new();
    for _ in 0..400 {
        let clock = clock.clone();

        j.push(tokio::spawn(async move {
            let conn = tonic::transport::Channel::from_shared(format!("http://{}", ADDR))
                .unwrap()
                .connect()
                .await
                .unwrap();

            let mut client = pb::echo_client::EchoClient::new(conn);

            for _ in 0..100 {
                let request = tonic::Request::new(pb::EchoRequest {
                    message: "hello".into(),
                });

                let start = clock.start();
                client.unary_echo(request).await.unwrap();
                let end = clock.end();

                let time_ms = clock.delta(start, end) / 1_000_000;
                metrics::timing!("request_duration_ms", time_ms);
            }
        }));
    }

    for j in j {
        j.await.unwrap();
    }

    Ok(())
}
