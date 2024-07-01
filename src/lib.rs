//! Metrics helper library for AWS Lambda Function.
//! Provides the way to put metrics to the `CloudWatch` using [EMF](https://docs.aws.amazon.com/AmazonCloudWatch/latest/monitoring/CloudWatch_Embedded_Metric_Format.html)
//!
//! # Examples
//! ```
//! async fn function_handler(event: LambdaEvent<Request>) -> Result<Response, Error> {
//!    let command = event.payload.command;
//!
//!    let mut metrics = Metrics::new("custom_lambdas", "service", "dummy_service");
//!
//!    metrics.try_add_dimension("application", "customer_service");
//!
//!    metrics.add_metric("test_count", MetricUnit::Count, 10.4);
//!
//!    metrics.add_metric("test_seconds", MetricUnit::Seconds, 15.0);
//!
//!    metrics.add_metric("test_count", MetricUnit::Count, 10.6);
//!
//!    // Prepare the response
//!    let resp = Response {
//!        req_id: event.context.request_id,
//!        msg: format!("Command {}.", command),
//!    };
//!
//!    Ok(resp)
//! }
//! ```
//!
//! Metrics are flushed automatically when the `Metrics` object is dropped.
//! Caller can flush metrics manually by calling `flush_metrics` method.
//!
//! ```
//! // ...
//!    let mut metrics = Metrics::new("custom_lambdas", "service", "dummy_service");
//!
//!    metrics.try_add_dimension("application", "customer_service");
//!
//!    metrics.add_metric("test_count", MetricUnit::Count, 10.4);
//!
//!    metrics.add_metric("test_seconds", MetricUnit::Seconds, 15.0);
//!
//!    metrics.flush_metrics()
//! // ...
//! ```
use std::collections::HashMap;

use chrono::Utc;
use serde::{Deserialize, Serialize};

const MAX_DIMENSIONS: usize = 30;
const MAX_METRICS: usize = 100;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct Dimensions(HashMap<String, String>);

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct MetricValues(HashMap<String, f64>);

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct DimensionName(String);

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Namespace(String);

/// `MetricDefinition` is used to serialize and publish metrics to `CloudWatch`.
/// List of units in the [AWS Documentation](https://docs.aws.amazon.com/AmazonCloudWatch/latest/APIReference/API_MetricDatum.html)
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum MetricUnit {
    Seconds,
    Microseconds,
    Milliseconds,
    Bytes,
    Kilobytes,
    Megabytes,
    Gigabytes,
    Terabytes,
    Count,
    BytesPerSecond,
    KilobytesPerSecond,
    MegabytesPerSecond,
    GigabytesPerSecond,
    TerabytesPerSecond,
    BitsPerSecond,
    KilobitsPerSecond,
    MegabitsPerSecond,
    GigabitsPerSecond,
    TerabitsPerSecond,
    CountPerSecond,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Metric {
    name: String,
    unit: MetricUnit,
    value: f64,
}

impl Metric {
    pub(crate) fn to_metric_definition(&self) -> MetricDefinition {
        MetricDefinition {
            name: self.name.clone(),
            unit: self.unit.clone(),
            storage_resolution: 60,
        }
    }
}

/// `Metrics` holds the current state of metrics to be logged to the `CloudWatch`.
/// It is eventually used to build internal `MetricDefinition` struct which is serialized and printed to the console
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Metrics {
    namespace: Namespace,
    dimensions: Dimensions,
    entries: Vec<Metric>,
}

impl Drop for Metrics {
    fn drop(&mut self) {
        println!("Dropping metrics, publishing metrics");
        self.flush_metrics();
    }
}

impl Metrics {
    /// Creates a new `Metrics` object with the given namespace and dimensions.
    #[allow(clippy::missing_panics_doc)]
    #[must_use]
    pub fn new(namespace: &str, dimension_key: &str, dimension_value: &str) -> Self {
        let mut metrics = Self {
            dimensions: Dimensions(HashMap::new()),
            namespace: Namespace(namespace.to_string()),
            entries: Vec::new(),
        };
        // UNWRAP: for new metrics there is no risk of reaching max number of dimensions
        metrics
            .try_add_dimension(dimension_key, dimension_value)
            .unwrap();
        metrics
    }
    /// Add new metric to the current `Metrics` object.
    /// - If metric's name is already present, the current metrics will be flushed and new metric will be added.
    /// - If the limit of `MAX_METRICS` is reached, the current metrics will be flushed automatically, and new metric will be added.
    pub fn add_metric(&mut self, name: &str, unit: MetricUnit, value: f64) {
        if self.entries.len() >= MAX_METRICS
            || self.entries.iter().any(|metric| metric.name == name)
        {
            self.flush_metrics();
        }
        self.entries.push(Metric {
            name: name.to_string(),
            unit,
            value,
        });
    }

    /// # Errors
    ///
    /// Will return `Err` if limit of `MAX_DIMENSION` is already reached
    /// The current limit is 30
    pub fn try_add_dimension(&mut self, key: &str, value: &str) -> Result<(), String> {
        if self.dimensions.0.len() >= MAX_DIMENSIONS {
            Err("Too many dimensions".into())
        } else {
            self.dimensions.0.insert(key.to_string(), value.to_string());
            Ok(())
        }
    }

    pub(crate) fn format_metrics(&self) -> CloudWatchMetricsLog {
        let metrics_definitions = self
            .entries
            .iter()
            .map(Metric::to_metric_definition)
            .collect::<Vec<MetricDefinition>>();

        let metrics_entries = vec![MetricDirective {
            namespace: self.namespace.0.to_string(),
            dimensions: vec![self
                .dimensions
                .0
                .keys()
                .map(|key| DimensionName(key.to_string()))
                .collect()],
            metrics: metrics_definitions,
        }];

        let cloudwatch_metrics = MetadataObject {
            timestamp: Utc::now().timestamp_millis(),
            cloud_watch_metrics: metrics_entries,
        };

        let metrics_values = self
            .entries
            .iter()
            .map(|metric| (metric.name.to_string(), metric.value))
            .collect::<HashMap<_, _>>();

        CloudWatchMetricsLog {
            aws: cloudwatch_metrics,
            dimensions: self.dimensions.clone(),
            metrics_values: MetricValues(metrics_values),
        }
    }

    /// Flushes the metrics to stdout in a single payload.
    /// # Errors
    /// 
    /// If an error occurs during serialization, it will be printed to stderr and won't be returned
    /// The function always successes
    pub fn flush_metrics(&mut self) {
        let serialized_metrics: Result<String, _> = self.format_metrics().try_into();

        match serialized_metrics {
            Ok(payload) => println!("{payload}"),
            Err(err) => eprintln!("Error when serializing metrics: {err}"),
        }
        self.entries = Vec::new();
    }
}

/// [MetricDefinition](https://docs.aws.amazon.com/AmazonCloudWatch/latest/monitoring/CloudWatch_Embedded_Metric_Format_Specification.html#CloudWatch_Embedded_Metric_Format_Specification_structure_metricdefinition)
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct MetricDefinition {
    name: String,
    unit: MetricUnit,
    storage_resolution: u64,
}

/// [MetricDirective](https://docs.aws.amazon.com/AmazonCloudWatch/latest/monitoring/CloudWatch_Embedded_Metric_Format_Specification.html#CloudWatch_Embedded_Metric_Format_Specification_structure_metricdirective)
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct MetricDirective {
    namespace: String,
    dimensions: Vec<Vec<DimensionName>>,
    metrics: Vec<MetricDefinition>,
}

/// [MetadataObject](https://docs.aws.amazon.com/AmazonCloudWatch/latest/monitoring/CloudWatch_Embedded_Metric_Format_Specification.html#CloudWatch_Embedded_Metric_Format_Specification_structure_metadata)
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct MetadataObject {
    timestamp: i64,
    cloud_watch_metrics: Vec<MetricDirective>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct CloudWatchMetricsLog {
    #[serde(rename = "_aws")]
    aws: MetadataObject,
    #[serde(flatten)]
    dimensions: Dimensions,
    #[serde(flatten)]
    metrics_values: MetricValues,
}

impl TryInto<String> for CloudWatchMetricsLog {
    type Error = String;

    fn try_into(self) -> Result<String, Self::Error> {
        serde_json::to_string(&self).map_err(|err| err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_create_metrics() {
        let mut metrics = Metrics::new("test_namespace", "service", "dummy_service");
        metrics.add_metric("test_metric_count", MetricUnit::Count, 1.0);
        metrics.add_metric("test_metric_seconds", MetricUnit::Seconds, 22.0);

        let log = metrics.format_metrics();

        assert_eq!(log.aws.cloud_watch_metrics[0].namespace, "test_namespace");
        assert_eq!(
            log.aws.cloud_watch_metrics[0].metrics[0].name,
            "test_metric_count"
        );
        assert_eq!(
            log.aws.cloud_watch_metrics[0].metrics[0].unit,
            MetricUnit::Count
        );
        assert_eq!(
            log.aws.cloud_watch_metrics[0].metrics[0].storage_resolution,
            60
        );
        assert_eq!(log.metrics_values.0.get("test_metric_count"), Some(&1.0));
        assert_eq!(
            log.aws.cloud_watch_metrics[0].metrics[1].name,
            "test_metric_seconds"
        );
        assert_eq!(
            log.aws.cloud_watch_metrics[0].metrics[1].unit,
            MetricUnit::Seconds
        );
        assert_eq!(
            log.aws.cloud_watch_metrics[0].metrics[1].storage_resolution,
            60
        );
        assert_eq!(log.dimensions.0.len(), 1);
    }

    #[test]
    fn should_handle_duplicated_metric() {
        let mut metrics = Metrics::new("test", "service", "dummy_service");
        metrics.add_metric("test", MetricUnit::Count, 2.0);
        metrics.add_metric("test", MetricUnit::Count, 1.0);

        assert_eq!(metrics.entries.len(), 1);
    }

    #[test]
    fn should_not_fail_over_100_metrics() {
        let mut metrics = Metrics::new("test", "service", "dummy_service");
        for i in 0..100 {
            metrics.add_metric(&format!("metric{i}"), MetricUnit::Count, i as f64);
        }

        assert_eq!(metrics.entries.len(), 100);
        metrics.add_metric("over_100", MetricUnit::Count, 11.0);
        assert_eq!(metrics.entries.len(), 1);
    }

    #[test]
    fn should_fail_if_over_30_dimensions() {
        let mut metrics = Metrics::new("test", "service", "dummy_service");
        for i in 0..29 {
            metrics
                .try_add_dimension(&format!("key{i}"), &format!("value{i}"))
                .unwrap();
        }

        match metrics.try_add_dimension("key31", "value31") {
            Ok(_) => assert!(false, "expected error"),
            Err(_) => assert!(true),
        }
    }
}
