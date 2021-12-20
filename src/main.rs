mod aws;
mod exporter;

use aws_config::Config;
use aws_sdk_ecs::model::{ContainerInstance, Resource, Service};
use color_eyre::Result;
use prometheus::{gather, opts, Encoder, IntGauge, IntGaugeVec, Registry, TextEncoder};
use tracing::warn;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    color_eyre::install()?;

    let config = aws_config::load_from_env().await;

    let registry = scrape(&config).await?;

    let mut buffer = vec![];
    let encoder = TextEncoder::new();
    let mut metric_families = gather();
    metric_families.extend(registry.gather());
    encoder
        .encode(&metric_families, &mut buffer)
        .expect("failed to encode metrics");
    println!("{}", String::from_utf8(buffer).unwrap());

    Ok(())
}

async fn scrape(aws_config: &Config) -> Result<Registry> {
    let client = aws_sdk_ecs::Client::new(aws_config);
    let ecs_client = aws::EcsClient::with_client(client);

    let registry = Registry::new();
    let status_opts = opts!(
        "aws_ecs_exporter_success",
        "Whether retrieval of ECS info was successful"
    );

    // holds the status of this particular scrape
    let status_gauge = IntGauge::with_opts(status_opts).expect("Failed to generate status gauge");

    let response = ecs_client.get_cluster_details("Tools").await;

    match response {
        Err(err) => warn!("{}", err),
        Ok(cluster_info) => {
            status_gauge.set(1);

            for mf in get_service_metrics(&cluster_info.services, "Tools") {
                registry
                    .register(Box::new(mf))
                    .expect("Failed to register services metrics");
            }

            for mf in get_instance_metrics(&cluster_info.instances, "Tools") {
                registry
                    .register(Box::new(mf))
                    .expect("Failed to register instances metrics");
            }
        }
    }
    registry
        .register(Box::new(status_gauge))
        .expect("Failed to register status gauge metric");

    Ok(registry)
}

fn get_service_metrics(services: &[Service], cluster_name: &str) -> Vec<IntGaugeVec> {
    let service_metric_family_current = IntGaugeVec::new(
        opts!(
            "aws_ecs_service_current_total",
            "Current Number of ECS Services"
        ),
        &["cluster_name", "service_name", "state"],
    )
    .expect("Failed to generate aws_ecs_service metric family");

    let service_metric_family_desired = IntGaugeVec::new(
        opts!("aws_ecs_service_desired", "Desired Number of ECS Services"),
        &["cluster_name", "service_name"],
    )
    .expect("Failed to generate aws_ecs_service metric family");

    for service in services {
        service_metric_family_desired
            .with_label_values(&[cluster_name, service.service_name.as_ref().unwrap()])
            .set(service.desired_count as i64);
        service_metric_family_current
            .with_label_values(&[
                cluster_name,
                service.service_name.as_ref().unwrap(),
                "running",
            ])
            .set(service.running_count as i64);
        service_metric_family_current
            .with_label_values(&[
                cluster_name,
                service.service_name.as_ref().unwrap(),
                "pending",
            ])
            .set(service.pending_count as i64);
    }

    vec![service_metric_family_desired, service_metric_family_current]
}

fn get_instance_metrics(instances: &[ContainerInstance], cluster_name: &str) -> Vec<IntGaugeVec> {
    let task_metric_family = IntGaugeVec::new(
        opts!(
            "aws_ecs_instance_tasks_total",
            "Tasks running on the Container Instances (ec2)"
        ),
        &["cluster_name", "ec2_instance_id", "state"],
    )
    .expect("Failed to generate aws_ecs_instance_tasks metric family");

    let resource_metric_family_registered = IntGaugeVec::new(
        opts!(
            "aws_ecs_instance_resources_registered",
            "Initial resources available on ECS Container Instance"
        ),
        &["cluster_name", "ec2_instance_id", "resource"],
    )
    .expect("Failed to generate aws_ecs_instance_resources_registered metric family");

    let resource_metric_family_remaining = IntGaugeVec::new(
        opts!(
            "aws_ecs_instance_resources_remaining",
            "Initial resources available on ECS Container Instance"
        ),
        &["cluster_name", "ec2_instance_id", "resource"],
    )
    .expect("Failed to generate aws_ecs_instance_resources_remaining metric family");

    for instance in instances {
        task_metric_family
            .with_label_values(&[
                cluster_name,
                instance.ec2_instance_id.as_ref().unwrap(),
                "running",
            ])
            .set(instance.running_tasks_count as i64);
        task_metric_family
            .with_label_values(&[
                cluster_name,
                instance.ec2_instance_id.as_ref().unwrap(),
                "pending",
            ])
            .set(instance.pending_tasks_count as i64);

        if let Some(remaining_resources) = &instance.remaining_resources {
            let resources: Vec<(&str, i64)> = remaining_resources
                .iter()
                .filter_map(filter_resources)
                .collect();
            for resource in resources {
                resource_metric_family_remaining
                    .with_label_values(&[
                        cluster_name,
                        instance.ec2_instance_id.as_ref().unwrap(),
                        resource.0,
                    ])
                    .set(resource.1);
            }
        }

        if let Some(registered_resources) = &instance.registered_resources {
            let resources: Vec<(&str, i64)> = registered_resources
                .iter()
                .filter_map(filter_resources)
                .collect();
            for resource in resources {
                resource_metric_family_registered
                    .with_label_values(&[
                        cluster_name,
                        instance.ec2_instance_id.as_ref().unwrap(),
                        resource.0,
                    ])
                    .set(resource.1);
            }
        }
    }

    vec![
        task_metric_family,
        resource_metric_family_registered,
        resource_metric_family_remaining,
    ]
}

fn filter_resources(resource: &Resource) -> Option<(&'static str, i64)> {
    match resource.name.as_deref() {
        Some("CPU") => Some(("cpu", resource.integer_value as i64)),
        Some("MEMORY") => Some(("ram", resource.integer_value as i64)),
        _ => None,
    }
}
