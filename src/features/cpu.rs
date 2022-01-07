use std::sync::Arc;
use crate::FeatureTrait;
use tokio::time::{ sleep, Duration };
use tokio::sync::mpsc;
use tokio::fs::read_to_string;
use std::process::{ Command, Stdio };
use std::io::prelude::*;
use crate::StatusBar;

pub struct Cpu {
    status_bar: Arc<StatusBar>,
    prefix: &'static str,
    idle: Duration,
    prev_total: usize,
    prev_idle: usize,
    cores: Vec<(usize, usize)>,
}

#[async_trait::async_trait]
impl FeatureTrait for Cpu {
    fn new(
        status_bar: Arc<StatusBar>,
        prefix: &'static str,
        idle: Duration,
    ) -> Self {
        Self {
            status_bar,
            prefix,
            idle,
            prev_total: 0,
            prev_idle: 0,
            cores: vec![],
        }
    }

    async fn pull(&mut self) {
        loop {
            let (usage, cores) = self._usage().await;
            let mut output = format!(
                "{}{:.1}% {}",
                self.prefix,
                usage,
                String::from_iter(cores)
            );

            if let Some(temp) = self._temperature().await {
                output = format!("{} {}", output, temp);
            }

            *self.status_bar.cpu.write().await = output;

            sleep(self.idle).await;
        }
    }
}

impl Cpu {
    async fn _temperature(&mut self) -> Option<String> {
        match Command::new("sensors")
            .output() {
            Ok(sensors) => {
                let grep = Command::new("grep")
                    .arg("Composite")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .spawn()
                    .unwrap();

                let mut output = String::new();
                grep.stdin.unwrap().write_all(&sensors.stdout).unwrap();
                grep.stdout.unwrap().read_to_string(&mut output).unwrap();

                Some(output.split_whitespace()
                    .skip(1)
                    .take(1)
                    .collect::<Vec<&str>>()
                    .join(""))
            },
            Err(_) => {
                eprintln!("'sensors' command not found!");
                None
            }
        }
    }

    async fn _usage(&mut self) -> (f64, Vec<char>) {
        let procstat = read_to_string("/proc/stat").await.unwrap();
        let mut usage = 0.0;
        let mut cores: Vec<char> = vec![];

        let _calc_perc_fn = |scale, total, idle, prev_total, prev_idle| -> f64 {
            scale * (1. - ((idle - prev_idle) as f64 / (total - prev_total) as f64))
        };

        procstat.split('\n')
            .filter(|line| line.starts_with("cpu"))
            .for_each(|line| {
                let fields: Vec<usize> = line.split_whitespace()
                    .skip(1)
                    .map(|field| field.parse::<usize>().unwrap())
                    .collect();

                let total: usize = fields.iter().sum();
                let idle = fields[3];

                if line.starts_with("cpu ") {
                    usage = _calc_perc_fn(100.0, total, idle, self.prev_total, self.prev_idle);
                    self.prev_total = total;
                    self.prev_idle = idle;
                } else {
                    let boxes = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█', '▉'];
                    let cpu = line.split_whitespace()
                        .take(1)
                        .collect::<Vec<&str>>()
                        .join("")
                        .replace("cpu", "")
                        .parse::<usize>()
                        .unwrap();

                    if self.cores.len() <= cpu {
                        self.cores.push((0, 0));
                    }

                    if cores.len() <= cpu {
                        cores.push(boxes[0]);
                    }

                    let perc = _calc_perc_fn(8.0, total, idle, self.cores[cpu].0, self.cores[cpu].1);

                    if (0.0..0.5).contains(&perc) { cores[cpu] = boxes[0]; }
                    if (0.5..1.5).contains(&perc) { cores[cpu] = boxes[1]; }
                    if (1.5..2.5).contains(&perc) { cores[cpu] = boxes[2]; }
                    if (2.5..3.5).contains(&perc) { cores[cpu] = boxes[3]; }
                    if (3.5..4.5).contains(&perc) { cores[cpu] = boxes[4]; }
                    if (4.5..5.5).contains(&perc) { cores[cpu] = boxes[5]; }
                    if (5.5..6.5).contains(&perc) { cores[cpu] = boxes[6]; }
                    if (6.5..7.5).contains(&perc) { cores[cpu] = boxes[7]; }
                    if (7.5..=8.0).contains(&perc) { cores[cpu] = boxes[8]; }

                    self.cores[cpu].0 = total;
                    self.cores[cpu].1 = idle;
                }
            });

        (usage, cores)
    }
}
