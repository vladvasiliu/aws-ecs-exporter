mod aws;
mod config;
mod exporter;

use crate::aws::EcsClient;
use crate::exporter::Exporter;
use color_eyre::Result;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    color_eyre::install()?;

    let config = aws_config::load_from_env().await;
    let aws_client = aws_sdk_ecs::client::Client::new(&config);
    let ecs_client = Arc::new(EcsClient::with_client(aws_client));

    let exporter = Exporter::new(([127, 0, 0, 1], 8080), None, ecs_client);
    exporter.work().await;

    Ok(())
}
