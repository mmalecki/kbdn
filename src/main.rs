#[macro_use]

use lazy_static::lazy_static;
use std::io::{self, BufRead};
use std::env;
use regex::Regex;

const PROGRESS_MAX: u8 = 63;

#[derive(Debug)]
struct ProgressUpdate {
    error: u8,
    success: u8,
}

const EMPTY_UPDATE: ProgressUpdate = ProgressUpdate {
    error: 0,
    success: 0
};

trait ProgressParser {
    fn new() -> Self where Self: Sized;
    fn parse_line(&mut self, line: &str) -> Option<ProgressUpdate>;
}

#[derive(Debug)]
struct TerraformParser {
    to_add: u16,
    to_change: u16,
    to_destroy: u16,
}

impl ProgressParser for TerraformParser {
    fn new() -> Self {
        TerraformParser {
            to_add: 0,
            to_change: 0,
            to_destroy: 0,
        }
    }

    fn parse_line(&mut self, line: &str) -> Option<ProgressUpdate> {
        lazy_static! {
            static ref PLAN_SUMMARY: Regex = Regex::new(r"(\d+) to add, (\d+) to change, (\d+) to destroy.").unwrap();
            static ref UPDATE: Regex = Regex::new(r"(Destruction|Creation) complete after").unwrap();
        }

        match PLAN_SUMMARY.captures(line) {
            Some(cg) => {
                self.to_add = cg[1].parse().unwrap();
                self.to_change = cg[2].parse().unwrap();
                self.to_destroy = cg[3].parse().unwrap();
                None
            },
            None => {
                let step = (PROGRESS_MAX as u16 / self.total()) as u8;

                match UPDATE.captures(line) {
                    Some(cg) => match &cg[1] {
                        "Creation" => Some(ProgressUpdate { success: step, error: 0 }),
                        "Destruction" => Some(ProgressUpdate { success: 0, error: step }),
                        _ => None,
                    },
                    None => None
                }
            }
        }
    }
}

impl TerraformParser {
    fn total(&self) -> u16 {
        self.to_add + self.to_change + self.to_destroy
    }
}

struct LinesParser {}

impl ProgressParser for LinesParser {
    fn new() -> Self {
        LinesParser {}
    }

    fn parse_line(&mut self, _line: &str) -> Option<ProgressUpdate> {
        Some(ProgressUpdate {
            error: 0,
            success: 1,
        })
    }
}

fn get_parser(spec: &str) -> Box<dyn ProgressParser> {
    match spec {
        "tf" => Box::new(TerraformParser::new()),
        "lines" | _ => Box::new(LinesParser::new()),
    }
}

#[async_std::main]
async fn main() -> surf::Result<()> {
    let args: Vec<String> = env::args().collect();

    let stdin = io::stdin();
    let mut progress = 0;

    let mut parser: Box<dyn ProgressParser> = get_parser(args[1].as_str());

    for line in stdin.lock().lines() {
        let update = parser.parse_line(&line.unwrap()).unwrap_or(EMPTY_UPDATE);

        progress = (progress + update.error + update.success) % (PROGRESS_MAX + 1);

        let res = surf::post("http://127.0.0.1:9916/progress/0")
            .body(surf::Body::from_string(progress.to_string()))
            .await?;
    }
    Ok(())
}
