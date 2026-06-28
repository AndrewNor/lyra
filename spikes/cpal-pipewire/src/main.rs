//! Phase-0 spike B: prove cpal builds + runs with the `pipewire` feature.
//! cpal 0.18.x: DeviceTrait::name() was REMOVED — Device impls Display;
//! use device.id()/device.description() for structured metadata.

use cpal::traits::{DeviceTrait, HostTrait};
use cpal::HostId;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Hosts compiled in : {:?}", cpal::ALL_HOSTS);
    println!("Hosts available   : {:?}", cpal::available_hosts());

    let host = cpal::host_from_id(HostId::PipeWire)
        .expect("PipeWire host unavailable - is the pipewire daemon running?");
    println!("Selected host     : {}", host.id().name());

    let device = host
        .default_output_device()
        .ok_or("no default output device on the PipeWire host")?;
    println!("Default output    : {device}");
    if let Ok(id) = device.id() {
        println!("Device id         : {id}");
    }

    let cfg = device.default_output_config()?;
    println!("Default out config: {cfg:?}");

    println!("\nAll PipeWire output devices:");
    for (i, dev) in host.output_devices()?.enumerate() {
        println!("  {}. {dev}", i + 1);
    }
    Ok(())
}
