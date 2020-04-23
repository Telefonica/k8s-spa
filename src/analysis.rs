use super::{ContainerId, MemoryInfo};
use log::info;
use std::collections::HashMap;

pub fn calculate_requests(memory_info: &MemoryInfo, tolerance: f64) -> (u64, HashMap<&ContainerId, u64>) {
  info!("Calculating requests...");
  let total_request_size = memory_info.global.value_at_quantile(1.0 - tolerance);
  let mut requests: HashMap<&ContainerId, u64> = HashMap::new();
  for i in 1..=100 {
    let total_request_at_percentile: u64 = memory_info.containers
      .values()
      .map(|h| h.value_at_percentile(i.into()))
      .sum();
    if total_request_at_percentile > total_request_size {
      requests = memory_info.containers.iter().map(|(container_id, h)| {
        (container_id, h.value_at_percentile((i - 1).into()))
      }).collect();
      break;
    }
  };
  (total_request_size, requests)
}
