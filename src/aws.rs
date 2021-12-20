use aws_sdk_ecs::model::{ContainerInstance, Failure, Service};
use color_eyre::Result;
use tracing::warn;

#[derive(Debug)]
pub struct ClusterInfo {
    pub services: Vec<Service>,
    pub instances: Vec<ContainerInstance>,
}

pub struct EcsClient {
    client: aws_sdk_ecs::Client,
}

impl EcsClient {
    pub fn with_client(client: aws_sdk_ecs::Client) -> Self {
        Self { client }
    }

    pub async fn get_cluster_details(&self, cluster: &str) -> Result<ClusterInfo> {
        let svc_list = self.get_service_names(cluster).await?;
        let services = self
            .get_services_details(cluster, svc_list.iter().map(String::as_ref).collect())
            .await?;

        let instance_name_list = self.get_container_instance_names(cluster).await?;
        let instances = self
            .get_container_instance_details(
                cluster,
                instance_name_list.iter().map(String::as_ref).collect(),
            )
            .await?;

        Ok(ClusterInfo {
            services,
            instances,
        })
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
