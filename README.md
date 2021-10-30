# Kubernetes Static Pod Autoscaler (k8s-spa)

- [Use cases](#use-cases)
- [Installation](#installation)
- [Usage](#usage)

k8s-spa is a command line utility that will automatically calculate memory requests for your pods based on:

1. real memory usage of your pods, based on Prometheus metrics
2. your risk tolerance (the higher your risk, the lower the requests)

It is similar to the [Vertical Pod Autoscaler](https://github.com/kubernetes/autoscaler/tree/master/vertical-pod-autoscaler) (VPA), in the sense that they both calculate requests for pods. However, there are several differences:

- k8s-spa calculates the requests _offline_, that is, there is no need to deploy anything to your Kubernetes cluster in order to use this tool.
- k8s-spa takes into account the memory metrics of all your pods in order to calculate the requests. This allows k8s-spa to further optimize the request values, since there isn't any harm in a pod using more memory than requested as long as there is slack from other pods that can absorb the excess memory usage. Additionally, k8s-spa is able to take into account memory correlations between pods


## Use cases
Who would benefit from using this tool?

- **K8s users that aren't autoscaling their pods**: instead of going through several cycles of trial and error to set the requests of your pods, you can just get a reliable request value by running k8s-spa.
- **People using the Horizontal Pod Autoscaler (HPA)**: The VPA doesn't play well with the HPA (Horizontal Pod Autoscaler) unless it is configured with custom metrics. You can use k8s-spa to automatically calculate your requests and still be able to use the HPA.

## Installation

### Source

Install cargo via [rustup](https://rustup.rs/).

```
cargo install k8s-spa
```

### Binary

Currently, the only binary distribution is through a docker container image:

```bash
docker run telefonica/k8s-spa:0.3.1
```

You can build the docker image with:

```bash
docker build -f Cargo.toml .
```

## Usage

There are two phases: importing data and analyzing data.

### Importing

Right now, we only support importing memory metrics from a [Prometheus](https://prometheus.io/) server:

```bash
k8s-spa import prometheus
        --end-date <end_date>
        --output <output>
        --start-date <start_date>
        --url <url>
```

In the future, other data sources could be implemented, like the Metric Server API from Kubernetes.

### Analyzing

Once you have imported your data into a file, you can analyze it to get the request values:

```bash
k8s-spa analyze --data <data> --risk-tolerance <risk>
```

The risk-tolerance parameter indicates how much risk of OOMKilled are you willing to accept for your particular scenario. The default value is 0.05 (i.e. 5%).

## Request calculation details

More documentation coming soon. For now, take a look at the code if you want to understand the gory details.