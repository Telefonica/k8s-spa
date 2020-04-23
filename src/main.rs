use chrono::{DateTime};
use clap::{Arg, App, crate_name, crate_version, SubCommand};
use hdrhistogram::Histogram;
use itertools::Itertools;
use pad::PadStr;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

mod analysis;
mod import;

#[derive(Debug, Ord, Eq, Hash, PartialOrd, PartialEq, Serialize, Deserialize)]
pub struct ContainerId {
  namespace: String,
  pod: String,
  container: String
}

pub struct MemoryInfo {
  global: Histogram<u32>,
  containers: HashMap<ContainerId, Histogram<u32>>
}

#[derive(Debug)]
struct Metrics {
  id: ContainerId,
  timestamp: u64,
  value: u32
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  env_logger::init();
  let matches = App::new(crate_name!())
    .version(crate_version!())
    .subcommand(SubCommand::with_name("import")
      .about("Imports data for analysis")
      .subcommand(SubCommand::with_name("prometheus")
        .arg(Arg::with_name("url")
          .long("url")
          .takes_value(true)
          .required(true)
          .help("Prometheus API URL (e.g. http://prometheus.example.com/api/"))
        .arg(Arg::with_name("user")
          .long("user")
          .short("u")
          .takes_value(true)
          .help("Basic Auth username"))
        .arg(Arg::with_name("pass")
          .long("password")
          .short("p")
          .takes_value(true)
          .requires("user")
          .help("Basic Auth password"))
        .arg(Arg::with_name("end_date")
          .long("end-date")
          .takes_value(true)
          .required(true)
          .help("End date for analysis, in ISO8601 format (e.g. 2020-04-19T19:00:00Z)"))
        .arg(Arg::with_name("start_date")
          .long("start-date")
          .takes_value(true)
          .required(true)
          .help("Start date for analysis, in ISO8601 format (e.g. 2020-04-19T21:00:00Z)"))
        .arg(Arg::with_name("output")
          .long("output")
          .short("o")
          .takes_value(true)
          .required(true)
          .help("File where to save or append data"))))
    .subcommand(SubCommand::with_name("analyze")
      .about("Analyzes imported data and offers requests suggestions")
      .arg(Arg::with_name("data")
        .long("data")
        .short("d")
        .takes_value(true)
        .required(true)
        .help("File with the imported data"))
      .arg(Arg::with_name("risk")
        .long("risk-tolerance")
        .short("r")
        .takes_value(true)
        .default_value("0.05")
        .help("The amount of OOM risk you want to take. This is a value between 0 and 1, \
                  where 0 means you want to avoid OOM at all costs (which will set the requests \
                  to the highest observed value for each pod).")))
    .get_matches();

  match matches.subcommand() {
    ("analyze", Some(args)) => {
      let memory_info = import::load_data(args.value_of("data").unwrap())?;
      let (total_size, requests) = analysis::calculate_requests(
        &memory_info,
        args.value_of("risk").unwrap().parse()?);
      println!("Total request size: {} MB", total_size);
      for (id, r) in requests.iter().sorted_by_key(|(id, _)| *id) {
        let (w, _) = term_size::dimensions().unwrap_or((80,0));
        let w = w.min(120);
        let memory_width = 6;
        let id = format!("{}/{}/{}", id.namespace, id.pod, id.container).pad_to_width_with_char(w-memory_width, '.');
        println!("{} {:>memory_width$}Mi", id, r, memory_width=memory_width-2);
      }
    }
    ("import", Some(args)) => {
      match args.subcommand() {
        ("prometheus", Some(args)) => {
          use import::*;
          let auth_info = args.value_of("user").map(|user| {
            BasicAuthInfo {
              user,
              pass: args.value_of("pass")
            }
          });
          let result = import_from_prometheus(
            args.value_of("url").unwrap(),
            auth_info,
            DateTime::parse_from_rfc3339(args.value_of("start_date").unwrap())?.timestamp(),
            DateTime::parse_from_rfc3339(args.value_of("end_date").unwrap())?.timestamp()
          )?;
          save_data(args.value_of("output").unwrap(), result)?;
        },
        _ => ()
      }
    },
    _ => println!("Use -h to see help")
  }
  Ok(())
}
