use std::thread;
use std::time::Duration;

use anyhow::Result;
use psutil::{cpu, disk, memory};
use psutil::Percent;

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

pub fn get_disk_usage() -> Result<Percent> {
    let disk = disk::disk_usage("/")?;
    Ok(disk.percent())
}
