use log::{debug, info};
use rayon::prelude::*;
use std::collections::HashMap;
use crate::{ContainerId, MemoryInfo};

pub fn calculate_requests(memory_info: &MemoryInfo, tolerance: f64) -> HashMap<&ContainerId, u64> {
  info!("Calculating requests...");
  let MemoryInfo {
    global: global_info,
    containers: containers_info,
    container_presence
  } = memory_info;
  let mut min: u8 = 0;
  let mut max: u8 = 100;
  while min < max {
    debug!("min {}, max {}", min, max);
    let percentile = (min + max) / 2;
    let under_requested_count = global_info
      .par_iter()
      .filter(|v| {
        let requests_at_ts: u64 = container_presence[&v.timestamp]
          .iter()
          .map(|id| containers_info[id].0.value_at_percentile(percentile as f64))
          .sum();
        requests_at_ts < v.value
      }).count();
    let risk = (under_requested_count as f64) / (global_info.len() as f64);
    if risk > tolerance {
      min = percentile + 1;
    } else {
      max = percentile;
    }
  };
  debug!("Percentile {}", min);
  containers_info
    .iter()
    .map(|(id, histogram)| (id, histogram.0.value_at_percentile(min as f64)))
    .collect()
}
