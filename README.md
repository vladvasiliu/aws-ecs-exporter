# AWS ECS Exporter
[![License](https://img.shields.io/github/license/vladvasiliu/aws-ecs-exporter.svg?style=flat)](COPYING)


A Prometheus exporter for AWS ECS status.

## Status
This exporter is in its very early stages. It is more or less just a PoC.

## Requirements
* An AWS account


## Usage
Pass the `-h` flag for a list of expected options.

```
aws-ecs-exporter --role=arn:aws:iam::123456789012:role/aws-ecs-exporter --cluster SomeCluster SomeOtherCluster --listen [::]:6543
```

### Behaviour

The exporter exposes two endpoints:

* `/status` can be used for a health check
* `/metrics` to gather the actual statistics


##  Building

This can be built on Linux, MacOS and Windows. As development happens on the latest stable versions of the Rust
toolchain and OS, there is no guarantee that older versions work.

```
cargo build --release
```

## Contributing

Any contributions are welcome. Please open an issue or PR if you find any bugs or would like to propose an enhancement.


## License

This project is released under the terms of the GNU General Public License, version 3.
Please see [`COPYING`](COPYING) for the full text of the license.
