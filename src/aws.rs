use crate::exporter::Scraper;
use async_trait::async_trait;
use aws_sdk_ecs::model::{Failure, Resource};
use color_eyre::Result;
use prometheus::{opts, IntGaugeVec, Registry};
use tracing::warn;

pub struct EcsClient {
    client: aws_sdk_ecs::Client,
    cluster_names: Vec<String>,
}

impl EcsClient {
    pub fn new<C: AsRef<str>>(client: aws_sdk_ecs::Client, cluster_names: &[C]) -> Self {
        Self {
            client,
            cluster_names: cluster_names
                .iter()
                .map(|x| x.as_ref().to_owned())
                .collect(),
        }
    }

    async fn get_service_names(&self, cluster_name: &str) -> Result<Vec<String>> {
        let mut next_token = None;
        let mut result = vec![];
        loop {
            let response = self
                .client
                .list_services()
                .cluster(cluster_name)
                .set_next_token(next_token)
                .send()
                .await?;
            if let Some(arn_vec) = response.service_arns {
                result.extend(arn_vec)
            }
            next_token = response.next_token;
            if next_token.is_none() {
                break;
            }
        }
        Ok(result)
    }

    /// Returns the details of the given services
    ///
    /// This will only return an `Err` if the request itself fails.
    /// In case of missing resources, it will only log the failures and the result will contain
    /// those resources which were found.
    async fn get_services_details(
        &self,
        cluster: &str,
        service_names: Vec<&str>,
    ) -> Result<Vec<aws_sdk_ecs::model::Service>> {
        let mut result = vec![];

        for chunk in service_names.chunks(10) {
            let response = self
                .client
                .describe_services()
                .cluster(cluster)
                .set_services(Some(chunk.iter().map(|x| x.to_string()).collect()))
                .send()
                .await?;
            log_failures(response.failures);
            if let Some(s) = response.services {
                result.extend(s);
            }
        }
        Ok(result)
    }

    async fn get_service_metrics(&self, cluster: &str) -> Result<Vec<IntGaugeVec>> {
        let svc_list = self.get_service_names(cluster).await?;
        let services = self
            .get_services_details(cluster, svc_list.iter().map(String::as_ref).collect())
            .await?;

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
                .with_label_values(&[cluster, service.service_name.as_ref().unwrap()])
                .set(service.desired_count as i64);
            service_metric_family_current
                .with_label_values(&[cluster, service.service_name.as_ref().unwrap(), "running"])
                .set(service.running_count as i64);
            service_metric_family_current
                .with_label_values(&[cluster, service.service_name.as_ref().unwrap(), "pending"])
                .set(service.pending_count as i64);
        }

        Ok(vec![
            service_metric_family_desired,
            service_metric_family_current,
        ])
    }

    async fn get_container_instance_names(&self, cluster_name: &str) -> Result<Vec<String>> {
        let mut next_token = None;
        let mut result = vec![];
        loop {
            let response = self
                .client
                .list_container_instances()
                .cluster(cluster_name)
                .set_next_token(next_token)
                .send()
                .await?;
            if let Some(arn_vec) = response.container_instance_arns {
                result.extend(arn_vec)
            }
            next_token = response.next_token;
            if next_token.is_none() {
                break;
            }
        }
        Ok(result)
    }

    async fn get_container_instance_details(
        &self,
        cluster: &str,
        instance_names: Vec<&str>,
    ) -> Result<Vec<aws_sdk_ecs::model::ContainerInstance>> {
        let mut result = vec![];

        for chunk in instance_names.chunks(10) {
            let response = self
                .client
                .describe_container_instances()
                .cluster(cluster)
                .set_container_instances(Some(chunk.iter().map(|x| x.to_string()).collect()))
                .send()
                .await?;
            log_failures(response.failures);
            if let Some(s) = response.container_instances {
                result.extend(s);
            }
        }
        Ok(result)
    }

    async fn get_container_instance_metrics(&self, cluster: &str) -> Result<Vec<IntGaugeVec>> {
        let instance_name_list = self.get_container_instance_names(cluster).await?;
        let instances = self
            .get_container_instance_details(
                cluster,
                instance_name_list.iter().map(String::as_ref).collect(),
            )
            .await?;

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
                    cluster,
                    instance.ec2_instance_id.as_ref().unwrap(),
                    "running",
                ])
                .set(instance.running_tasks_count as i64);
            task_metric_family
                .with_label_values(&[
                    cluster,
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
                            cluster,
                            instance.ec2_instance_id.as_ref().unwrap(),
                            resource.0,
                        ])
                        .set(resource.1);
                }
            }

            // TODO: Properly handle all resource types
            if let Some(registered_resources) = &instance.registered_resources {
                let resources: Vec<(&str, i64)> = registered_resources
                    .iter()
                    .filter_map(filter_resources)
                    .collect();
                for resource in resources {
                    resource_metric_family_registered
                        .with_label_values(&[
                            cluster,
                            instance.ec2_instance_id.as_ref().unwrap(),
                            resource.0,
                        ])
                        .set(resource.1);
                }
            }
        }

        Ok(vec![
            task_metric_family,
            resource_metric_family_registered,
            resource_metric_family_remaining,
        ])
    }
}

#[async_trait]
impl Scraper for EcsClient {
    async fn scrape(&self) -> Result<Registry> {
        let registry = Registry::new();
        let scrape_metric = IntGaugeVec::new(
            opts!(
                "aws_ecs_cluster_scrape_success",
                "Whether the scrape for a particular cluster and resource kind was successful"
            ),
            &["cluster_name", "scraped_resource"],
        )
        .expect("Failed to generate aws_ecs_cluster_scrape_success metric");

        for cluster_name in &self.cluster_names {
            let instance_scrape_metric =
                scrape_metric.with_label_values(&[&cluster_name, "cluster_instances"]);
            match self.get_container_instance_metrics(&cluster_name).await {
                Ok(instance_metrics) => {
                    for mf in instance_metrics {
                        registry
                            .register(Box::new(mf))
                            .expect("Failed to register instances metrics");
                    }
                    instance_scrape_metric.set(1);
                }
                Err(err) => {
                    warn!(
                        "Failed to get instance metrics for cluster `{}`: {}",
                        cluster_name, err
                    );
                }
            }

            let service_scrape_metric =
                scrape_metric.with_label_values(&[&cluster_name, "services"]);
            match self.get_service_metrics(&cluster_name).await {
                Ok(service_metrics) => {
                    for mf in service_metrics {
                        registry
                            .register(Box::new(mf))
                            .expect("Failed to register services metrics");
                    }
                    service_scrape_metric.set(1);
                }
                Err(err) => warn!(
                    "Failed to get service metrics for cluster `{}`: {}",
                    cluster_name, err
                ),
            }
        }

        registry
            .register(Box::new(scrape_metric))
            .expect("Failed to register aws_ecs_cluster_scrape_success metric");
        Ok(registry)
    }
}

fn filter_resources(resource: &Resource) -> Option<(&'static str, i64)> {
    match resource.name.as_deref() {
        Some("CPU") => Some(("cpu", resource.integer_value as i64)),
        Some("MEMORY") => Some(("ram", resource.integer_value as i64)),
        _ => None,
    }
}

fn log_failures(failures: Option<Vec<Failure>>) {
    if let Some(failures) = failures {
        for failure in failures {
            warn!(
                failure.arn = failure.arn.as_deref(),
                failure.reason = failure.reason.as_deref(),
                failure.detail = failure.detail.as_deref(),
                "Failed to describe service"
            );
        }
    }
}
