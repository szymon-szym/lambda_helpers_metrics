use lambda_runtime::{run, service_fn, tracing, Error, LambdaEvent};

use lambda_helpers_metrics::{MetricUnit, Metrics};
use serde::{Deserialize, Serialize};

/// This is a made-up example. Requests come into the runtime as unicode
/// strings in json format, which can map to any structure that implements `serde::Deserialize`
/// The runtime pays no attention to the contents of the request payload.
#[derive(Deserialize)]
struct Request {
    command: String,
}

/// This is a made-up example of what a response structure may look like.
/// There is no restriction on what it can be. The runtime requires responses
/// to be serialized into json. The runtime pays no attention
/// to the contents of the response payload.
#[derive(Serialize)]
struct Response {
    req_id: String,
    msg: String,
}

async fn function_handler(event: LambdaEvent<Request>) -> Result<Response, Error> {
    // Extract some useful info from the request
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

    // Return `Response` (it will be serialized to JSON automatically by the runtime)
    Ok(resp)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(service_fn(function_handler)).await
}
