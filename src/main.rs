mod aws;
mod config;
mod exporter;

use crate::aws::{get_credentials_provider, EcsClient};
use crate::exporter::Exporter;
use aws_config::meta::region::RegionProviderChain;
use aws_types::credentials::SharedCredentialsProvider;
use color_eyre::Result;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    color_eyre::install()?;

    let config = config::Config::from_args();

    let region = RegionProviderChain::first_try(config.region)
        .or_default_provider()
        .region()
        .await
        .expect("Failed to determine region");

    let default_credentials_provider =
        aws_config::default_provider::credentials::default_provider().await;

    let mut aws_config_loader = aws_config::from_env().region(region.clone());

    if let Some(role) = config.aws_role {
        let default_credentials_provider =
            SharedCredentialsProvider::new(default_credentials_provider);
        let cp = get_credentials_provider(default_credentials_provider, &role, None, None, region);
        aws_config_loader = aws_config_loader.credentials_provider(cp);
    };

    let aws_config = aws_config_loader.load().await;

    let aws_client = aws_sdk_ecs::client::Client::new(&aws_config);
    let ecs_client = Arc::new(EcsClient::new(aws_client, &config.cluster_names));

    let exporter = Exporter::new(
        config.listen_address,
        None,
        ecs_client,
        "aws_ecs_exporter",
        &config.app_version,
    );
    exporter.work().await;

    Ok(())
}
