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

    let config = config::Config::from_args();

    let aws_config = aws_config::load_from_env().await;
    let aws_client = aws_sdk_ecs::client::Client::new(&aws_config);
    let ecs_client = Arc::new(EcsClient::new(aws_client, &config.cluster_names));

    let exporter = Exporter::new(config.listen_address, None, ecs_client);
    exporter.work().await;

    Ok(())
}
