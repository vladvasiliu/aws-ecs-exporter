use crate::aws::EcsClient;
use crate::config::TlsConfig;
use async_trait::async_trait;
use color_eyre::Result;
use lazy_static::lazy_static;
use prometheus::{gather, opts, register, Encoder, IntCounterVec, IntGauge, Registry, TextEncoder};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::warn;
use warp::{Filter, Reply};

#[async_trait]
pub trait Scraper: Send + Sync {
    async fn scrape(&self) -> Result<Registry>;
}

pub struct Exporter {
    socket_address: SocketAddr,
    tls_config: Option<TlsConfig>,
    scraper: Arc<dyn Scraper>, // A wrapper around an aws-sdk-ecs client
    exporter_metrics: Arc<IntCounterVec>, // Metrics that concern the exporter itself
}

impl Exporter {
    pub fn new(
        socket_address: impl Into<SocketAddr>,
        tls_config: Option<TlsConfig>,
        scraper: EcsClient,
    ) -> Self {
        let scraper = Arc::new(scraper);

        let exporter_opts = opts!(
            "http_requests",
            "Number of HTTP requests received by the exporter"
        );
        let exporter_metrics = IntCounterVec::new(exporter_opts, &["status"])
            .expect("Failed to create exporter metrics");
        register(Box::new(exporter_metrics.clone()))
            .expect("Failed to register exporter metrics family");

        Self {
            socket_address: socket_address.into(),
            tls_config,
            scraper,
            exporter_metrics: Arc::new(exporter_metrics),
        }
    }

    pub async fn work(&self) {
        let scraper = self.scraper.clone();
        let exporter_metrics = self.exporter_metrics.clone();
        let metrics = warp::path("metrics")
            .and_then(move || scrape(scraper.clone(), exporter_metrics.clone()));

        let home = warp::path::end().map(|| warp::reply::html(HOME_PAGE.as_str()));
        let status = warp::path("status").map(|| warp::reply::html(STATUS_PAGE));
        let route = home.or(status).or(metrics);

        let server = warp::serve(route);
        match &self.tls_config {
            Some(tls_config) => {
                let server = server
                    .tls()
                    .key_path(&tls_config.key)
                    .cert_path(&tls_config.cert);
                server.bind(self.socket_address).await;
            }
            None => server.try_bind(self.socket_address).await,
        }
    }
}

// Separate function helps with async lifetime requirements
async fn scrape(
    scraper: Arc<dyn Scraper>,
    exporter_metrics_family: Arc<IntCounterVec>,
) -> std::result::Result<impl Reply, Infallible> {
    let status_opts = opts!(
        "aws_ecs_exporter_success",
        "Whether retrieval of ECS events from AWS API was successful"
    );
    // holds the status of this particular scrape
    let status_gauge = IntGauge::with_opts(status_opts).expect("Failed to generate status gauge");

    // The match sets the label to increment for the http metric, either success or error
    // Status gauge represents the status of only this particular scrape
    let labels: &[&str];

    // This registry contains the metrics for this particular scrape
    let registry = match scraper.scrape().await {
        Ok(registry) => {
            status_gauge.set(1);
            labels = &["success"];
            registry
        }
        Err(err) => {
            warn!("{}", err);
            labels = &["error"];
            Registry::new()
        }
    };

    registry.register(Box::new(status_gauge)).unwrap();
    exporter_metrics_family
        .get_metric_with_label_values(labels)
        .unwrap()
        .inc();

    let mut buffer = vec![];
    let encoder = TextEncoder::new();

    let mut metric_families = gather(); // Gather the common metrics family
    metric_families.extend(registry.gather()); // Add the metrics from this particular scrape
    encoder.encode(&metric_families, &mut buffer).unwrap();
    Ok(String::from_utf8(buffer).unwrap())
}

lazy_static! {
    static ref HOME_PAGE: String = format!(
        "
        <html>
        <head><title>AWS ECS Exporter</title></head>
        <body>
            AWS ECS Exporter v{}
            <ul>
                <li><a href=\"/status\">Exporter status</a></li>
                <li><a href=\"/metrics\">Metrics</a></li>
            </ul>
        </body>
    </html>
    ",
        // crate_version!()
        1.1
    );
}

static STATUS_PAGE: &str =
    "<html><head><title>AWS ECS Exporter</title></head><body>Ok</body></html>";
