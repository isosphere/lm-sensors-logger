use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

use chrono::prelude::*;
use clap::Parser;
use lazy_regex::regex;
use rusqlite::Connection;

const SENSORS_PATH: &str = "/usr/bin/sensors";
const DATABASE_PATH: &str = "sensors.db";

#[derive(Parser)]
struct Arguments {
    /// path to the sensors binary
    #[clap(short, long, default_value = SENSORS_PATH)]
    sensors_path: String,

    /// path to the SQLite database file to store sensor values
    #[clap(short, long, default_value = DATABASE_PATH)]
    database_path: String,

    /// interval in seconds to poll the sensors
    #[clap(short, long, default_value = "5")]
    poll_interval: u64,
}

#[derive(Debug)]
struct SensorValue {
    device: String,
    label: String,
    value: f64,
    units: String,
}

fn poll_sensors(bin_path: &str) -> Vec<SensorValue> {
    let sensor_regex =
        regex!(r"^(?P<label>[^:]+):\s+(?P<value>(\+|-)?\d+(\.\d+)?)\s*(?P<units>[^0-9 ]+).*");

    let output = match Command::new(bin_path).output() {
        Ok(o) => {
            String::from_utf8(o.stdout).expect("Invalid UTF-8 sequence in sensors binary output.")
        }
        Err(e) => panic!("Error executing sensors binary: {}", e),
    };

    let mut sensor_values = Vec::new();

    let mut device = None;
    for line in output.lines() {
        if device.is_none() {
            device = Some(line);
            continue;
        } else if line.is_empty() {
            device = None;
            continue;
        }

        if let Some(caps) = sensor_regex.captures(line) {
            let label = caps.name("label").unwrap().as_str();
            let value = caps.name("value").unwrap().as_str();
            let units = caps.name("units").unwrap().as_str();

            sensor_values.push(SensorValue {
                device: device.unwrap().to_string(),
                label: label.to_string(),
                value: value.parse().unwrap(),
                units: units.to_string(),
            });
        }
    }

    sensor_values
}

fn main() {
    let args = Arguments::parse();

    let conn = Connection::open(&args.database_path).expect("Failed to open database.");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sensor_values (
            datetime TEXT NOT NULL,
            device TEXT NOT NULL,
            label TEXT NOT NULL,
            value REAL NOT NULL,
            units TEXT NOT NULL
        )",
        [],
    )
    .expect("Failed to create table.");

    let mut stmt = conn
        .prepare(
            "INSERT INTO sensor_values (datetime, device, label, value, units)
            VALUES (?, ?, ?, ?, ?)",
        )
        .expect("Failed to prepare statement.");

    loop {
        let sensor_values = poll_sensors(&args.sensors_path);
        let datetime = Utc::now();

        for sensor_value in sensor_values.iter() {
            stmt.execute(&[
                &datetime.to_rfc3339(),
                &sensor_value.device,
                &sensor_value.label,
                &sensor_value.value.to_string(),
                &sensor_value.units,
            ])
            .expect("Failed to insert sensor value.");
        }

        sleep(Duration::from_secs(args.poll_interval));
    }
}
