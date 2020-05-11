use hdrhistogram::Histogram;
use log::info;
use regex::Regex;
use reqwest::blocking::Client;
use rmp_serde::{from_slice, to_vec};
use serde::Serialize;
use std::{
  collections::HashMap,
  fs::File,
  io::prelude::*
};
use serde_json::Value;
use crate::{ContainerId, MemoryInfo, MetricValue, SerializableHistogram, ControllerType};

pub struct BasicAuthInfo<'a> {
  pub user: &'a str,
  pub pass: Option<&'a str>
}

#[derive(Serialize, Debug)]
struct PrometheusRangeQuery {
  query: String,
  start: i64,
  end: i64,
  step: u32
}

fn query_prometheus(client: &Client,
                    url: &str,
                    basic_auth: &Option<BasicAuthInfo>,
                    query: PrometheusRangeQuery) -> Result<Value, Box<dyn std::error::Error>> {
  let req = client.post(url);
  let req = if let Some(basic_auth) = basic_auth {
    req.basic_auth(basic_auth.user, basic_auth.pass)
  } else {
    req
  };
  Ok(req
    .form(&query)
    .send()?
    .json()?)
}

pub fn import_from_prometheus(url: &str,
                              basic_auth: Option<BasicAuthInfo>,
                              start: i64,
                              end: i64) -> Result<MemoryInfo, Box<dyn std::error::Error>> {
  let client = Client::new();
  let url = format!("{}/v1/query_range", url).replace("//", "/");
  info!("Getting container metrics from Prometheus...");
  let query = PrometheusRangeQuery {
    query: "container_memory_working_set_bytes{container!=\"\", container!=\"POD\"}".to_string(),
    start,
    end,
    step: 15
  };
  let mut containers = HashMap::new();
  let resp = query_prometheus(&client, &url, &basic_auth, query)?;
  let resp = resp["data"]["result"]
    .as_array()
    .unwrap()
    .into_iter()
    .filter(|v| v["metric"].get("pod").is_some());
  let mut container_presence = HashMap::new();
  let is_deployment = Regex::new(r"^[\w-]+-[0-9a-f]+-[a-z0-9]{5}$").unwrap();
  let is_daemonset = Regex::new(r"^[\w-]+-[a-z0-9]{5}$").unwrap();
  let is_statefulset = Regex::new(r"^[\w-]+-[0-9]$").unwrap();
  for metric_info in resp {
    let pod = metric_info["metric"]["pod"].as_str().unwrap();

    let (controller_id, controller_type) = if is_deployment.is_match(pod) {
      (&pod[..pod.len()-17], ControllerType::DEPLOYMENT)
    } else if is_daemonset.is_match(pod) {
      (&pod[..pod.len()-6], ControllerType::DAEMONSET)
    } else if is_statefulset.is_match(pod) {
      (&pod[..pod.len()-2], ControllerType::STATEFULSET)
    } else {
      (pod, ControllerType::OTHER)
    };
    let id = ContainerId {
      namespace: metric_info["metric"]["namespace"].as_str().unwrap().to_string(),
      controller_type,
      controller_id: controller_id.to_string(),
      container: metric_info["metric"]["container"].as_str().unwrap().to_string(),
    };
    let mut container_histogram = Histogram::new(3)?;
    let metrics = metric_info["values"]
      .as_array()
      .unwrap()
      .iter()
      .map(|v| {
        let info = v.as_array().unwrap();
        let timestamp: u64 = info[0].as_u64().unwrap();
        let memory_bytes: u64 = info[1].as_str().unwrap().parse().unwrap();
        let value = memory_bytes / (1024*1024);
        MetricValue { timestamp, value }
      });
    for v in metrics {
      container_histogram.record(v.value)?;
      let presence_set = container_presence.entry(v.timestamp).or_insert(Vec::new());
      presence_set.push(id.clone());
    }
    if let Some(SerializableHistogram(h)) = containers.get_mut(&id) {
      h.add(container_histogram).unwrap();
    } else {
      containers.insert(id, SerializableHistogram(container_histogram));
    }
  }
  info!("Getting global metrics from Prometheus...");
  let query = PrometheusRangeQuery {
    query: "sum(container_memory_working_set_bytes{container!=\"\", container!=\"POD\"})".to_string(),
    start,
    end,
    step: 15
  };
  let resp = query_prometheus(&client, &url, &basic_auth, query)?;
  let resp = resp["data"]["result"]
    .as_array()
    .unwrap()
    .into_iter();
  let metrics: Vec<_> = resp.into_iter().flat_map(|metric_info| {
    metric_info["values"]
      .as_array()
      .unwrap()
      .into_iter()
      .map(|v| {
        let info = v.as_array().unwrap();
        let timestamp: u64 = info[0].as_u64().unwrap();
        let memory_bytes: u64 = info[1].as_str().unwrap().parse().unwrap();
        let value = memory_bytes / (1024*1024);
        MetricValue { timestamp, value }
      })
  }).collect();
  Ok(MemoryInfo {
    global: metrics,
    containers,
    container_presence
  })
}

pub fn save_data(file_path: &str, data: MemoryInfo) -> Result<(), Box<dyn std::error::Error>> {
  info!("Saving data to disk...");
  let mut file = File::create(file_path)?;
  file.write_all(&to_vec(&data)?)?;
  Ok(())
}

pub fn load_data(file_path: &str) -> Result<MemoryInfo, Box<dyn std::error::Error>> {
  info!("Loading data form disk...");
  let mut file = File::open(file_path)?;
  let mut data = vec![];
  file.read_to_end(&mut data)?;
  Ok(from_slice(&data)?)
}
