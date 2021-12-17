mod aws;

use color_eyre::Result;
use tracing::warn;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    color_eyre::install()?;

    let config = aws_config::load_from_env().await;
    let client = aws_sdk_ecs::Client::new(&config);
    let ecs_client = aws::EcsClient::with_client(client);

    let response = ecs_client.list_services(&["Tools"]).await?;
    println!("{:#?}", response);

    Ok(())
}
