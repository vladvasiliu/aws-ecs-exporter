use aws_types::region::Region;
use clap::{app_from_crate, AppSettings, Arg};
use regex::Regex;
use std::net::SocketAddr;

#[derive(Debug)]
pub struct TlsConfig {
    pub key: String,
    pub cert: String,
}

#[derive(Debug)]
pub struct Config {
    pub cluster_names: Vec<String>,
    pub aws_role: Option<String>,
    pub listen_address: SocketAddr,
    pub region: Option<Region>,
}

impl Config {
    pub fn from_args() -> Self {
        let role_re: Regex = Regex::new(r"(?i:arn:aws:iam::\d{12}:role/.*)").unwrap();
        let matches = app_from_crate!()
            .setting(AppSettings::DeriveDisplayOrder)
            .term_width(120)
            .args(&[
                Arg::new("clusters")
                    .long("cluster")
                    .takes_value(true)
                    .value_name("CLUSTER")
                    .required(true)
                    .multiple_occurrences(true)
                    .multiple_values(true)
                    .forbid_empty_values(true)
                    .env("ECS_EXPORTER_CLUSTERS")
                    .help("Cluster name (one or more)"),
                Arg::new("region")
                    .long("region")
                    .takes_value(true)
                    .value_name("AWS_REGION")
                    .required(false)
                    .multiple_occurrences(false)
                    .multiple_values(false)
                    .forbid_empty_values(true)
                    .env("AWS_REGION")
                    .help("AWS Region to use, if any"),
                Arg::new("role")
                    .long("role")
                    .takes_value(true)
                    .value_name("AWS_ROLE")
                    .required(false)
                    .multiple_occurrences(false)
                    .multiple_values(false)
                    .forbid_empty_values(true)
                    .env("ECS_EXPORTER_ROLE")
                    .validator_regex(
                        role_re,
                        "must be of the form `arn:aws:iam::123456789012:role/something`",
                    )
                    .help("AWS Role to assume, if any"),
                Arg::new("listen")
                    .short('l')
                    .long("listen")
                    .takes_value(true)
                    .value_name("LISTEN")
                    .required(false)
                    .multiple_occurrences(false)
                    .multiple_values(false)
                    .forbid_empty_values(true)
                    .env("ECS_EXPORTER_LISTEN")
                    .default_value("[::1]:6543")
                    .validator(validate_listen_address)
                    .help("HTTP listen address"),
            ])
            .get_matches();

        Self {
            cluster_names: matches.values_of_t_or_exit("clusters"),
            aws_role: matches.value_of("role").map(String::from),
            listen_address: matches.value_of_t_or_exit("listen"),
            region: matches
                .value_of("region")
                .map(String::from)
                .map(Region::new),
        }
    }
}

fn validate_listen_address(value: &str) -> Result<(), String> {
    value
        .parse::<SocketAddr>()
        .map_err(|err| format!("{}", err))
        .map(|_| ())
}
