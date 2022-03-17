use std::thread;
use std::time::Duration;

use psutil::{cpu, memory};
use psutil::Percent;
use anyhow::Result;

pub fn get_cpu_usage() -> Result<Percent> {
    let block_time = Duration::from_millis(1000);
    let mut collector = cpu::CpuPercentCollector::new().unwrap();
    thread::sleep(block_time);
    Ok(collector.cpu_percent()?)
}

pub fn get_mem_usage() -> Result<Percent> {
    let mem = memory::virtual_memory()?;
    Ok(mem.percent())
}
