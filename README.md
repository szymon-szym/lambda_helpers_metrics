# Metrics helper library for AWS Lambda Function

Provides the way to put metrics to the `CloudWatch` using [EMF](https://docs.aws.amazon.com/AmazonCloudWatch/latest/monitoring/CloudWatch_Embedded_Metric_Format.html)

# Examples
```Rust
async fn function_handler(event: LambdaEvent<Request>) -> Result<Response, Error> {
    let command = event.payload.command;

    let mut metrics = Metrics::new("custom_lambdas", "service", "dummy_service");

    metrics.try_add_dimension("application", "customer_service");

    metrics.add_metric("test_count", MetricUnit::Count, 10.4);

    metrics.add_metric("test_seconds", MetricUnit::Seconds, 15.0);

    metrics.add_metric("test_count", MetricUnit::Count, 10.6);

    // Prepare the response
    let resp = Response {
        req_id: event.context.request_id,
        msg: format!("Command {}.", command),
    };

    Ok(resp)
}
```

Metrics are flushed automatically when the `Metrics` object is dropped.

Caller can flush metrics manually by calling `flush_metrics` method.

```Rust
// ...
let mut metrics = Metrics::new("custom_lambdas", "service", "dummy_service");

metrics.try_add_dimension("application", "customer_service");

metrics.add_metric("test_count", MetricUnit::Count, 10.4);

metrics.add_metric("test_seconds", MetricUnit::Seconds, 15.0);

metrics.flush_metrics()
// ...
```