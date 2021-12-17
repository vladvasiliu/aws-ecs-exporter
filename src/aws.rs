use aws_sdk_ecs::model::{Failure, Service};
use color_eyre::Result;
use std::collections::HashMap;
use tracing::warn;

pub struct EcsClient {
    client: aws_sdk_ecs::Client,
}

impl EcsClient {
    pub fn with_client(client: aws_sdk_ecs::Client) -> Self {
        Self { client }
    }

    pub async fn list_services(&self, clusters: &[&str]) -> Result<HashMap<String, Vec<Service>>> {
        let mut result = HashMap::new();

        for cluster in clusters {
            let svc_list = self.get_service_names(cluster).await?;
            let svc = self
                .get_services_details(cluster, svc_list.iter().map(String::as_ref).collect())
                .await?;
            result.insert(cluster.to_string(), svc);
        }
        Ok(result)
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
