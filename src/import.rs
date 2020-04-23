use hdrhistogram::{Histogram, serialization::{Serializer, V2DeflateSerializer}};
use log::info;
use reqwest::blocking::Client;
use rmp_serde::{from_slice, to_vec};
use serde::{Deserialize, Serialize, de::Visitor};
use std::{
  collections::HashMap,
  fs::File,
  io::prelude::*
};
use super::{ContainerId, MemoryInfo};
use serde_json::Value;

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
  for metric_info in resp {
    let id = ContainerId {
      namespace: metric_info["metric"]["namespace"].as_str().unwrap().to_string(),
      pod: metric_info["metric"]["pod"].as_str().unwrap().to_string(),
      container: metric_info["metric"]["container"].as_str().unwrap().to_string(),
    };
    let mut container_histogram = Histogram::new(3)?;
    for v in metric_info["values"].as_array().unwrap() {
      let info = v.as_array().unwrap();
      let memory_bytes: u64 = info[1].as_str().unwrap().parse().unwrap();
      let value = memory_bytes / (1024*1024);
      container_histogram.record(value)?;
    }
    containers.insert(id, container_histogram);
  }
  info!("Getting global metrics from Prometheus...");
  let mut global = Histogram::new(4)?;
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
  for metric_info in resp {
    for v in metric_info["values"].as_array().unwrap() {
      let info = v.as_array().unwrap();
      let memory_bytes: u64 = info[1].as_str().unwrap().parse().unwrap();
      let value = memory_bytes / (1024*1024);
      global.record(value)?;
    }
  }
  Ok(MemoryInfo {
    global,
    containers
  })
}

struct HistogramSerialization(Histogram<u32>);

impl Serialize for HistogramSerialization {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
      let mut vec = Vec::new();
      V2DeflateSerializer::new().serialize(&self.0, &mut vec).unwrap();
      Ok(serializer.serialize_bytes(&vec)?)
    }
}

impl<'de> Deserialize<'de> for HistogramSerialization {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
      struct BytesVisitor;

      impl<'a> Visitor<'a> for BytesVisitor {
        type Value = &'a [u8];

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
          formatter.write_str("a borrowed byte array")
        }

        fn visit_borrowed_str<E>(self, v: &'a str) -> Result<Self::Value, E>
        where E: serde::de::Error,
        {
          Ok(v.as_bytes())
        }

        fn visit_borrowed_bytes<E>(self, v: &'a [u8]) -> Result<Self::Value, E>
        where E: serde::de::Error,
        {
          Ok(v)
        }
      }
      let mut bytes = std::io::Cursor::new(deserializer.deserialize_bytes(BytesVisitor)?);
      let histogram = hdrhistogram::serialization::Deserializer::new().deserialize(&mut bytes).unwrap();
      Ok(HistogramSerialization(histogram))
    }
}

#[derive(Serialize, Deserialize)]
struct MemoryInfoSerialization {
  global: HistogramSerialization,
  containers: HashMap<ContainerId, HistogramSerialization>
}

pub fn save_data(file_path: &str, data: MemoryInfo) -> Result<(), Box<dyn std::error::Error>> {
  info!("Saving data to disk...");
  let data = MemoryInfoSerialization {
    global: HistogramSerialization(data.global),
    containers: data.containers
      .into_iter()
      .map(|(k, v)| (k, HistogramSerialization(v)))
      .collect()
  };
  let mut file = File::create(file_path)?;
  file.write_all(&to_vec(&data)?)?;
  Ok(())
}

pub fn load_data(file_path: &str) -> Result<MemoryInfo, Box<dyn std::error::Error>> {
  info!("Loading data form disk...");
  let mut file = File::open(file_path)?;
  let mut data = vec![];
  file.read_to_end(&mut data)?;
  let data: MemoryInfoSerialization = from_slice(&data)?;
  Ok(MemoryInfo {
    global: data.global.0,
    containers: data.containers.into_iter().map(|(k, v)| (k, v.0)).collect()
  })
}
