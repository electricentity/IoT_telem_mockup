use clap::{App, Arg};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
    thread,
    time::Duration,
};

#[derive(Serialize, Deserialize, Debug)]
enum Severity {
    Debug,
    Info,
    Warning,
    Error,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
enum MessageType {
    Log,
    SensorData,
}

#[derive(Serialize, Deserialize, Debug)]
struct LogMessage {
    severity: Severity,
    message: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct SensorData {
    name: String,
    value: f32,
}

#[derive(Serialize, Deserialize, Debug)]
struct Message {
    timestamp: i64,
    device: String,
    firmware: String,
    message_type: MessageType,
    #[serde(default)] // Makes the field optional during deserialization
    #[serde(skip_serializing_if = "Option::is_none")]
    log_message: Option<LogMessage>,
    #[serde(default)] // Makes the field optional during deserialization
    #[serde(skip_serializing_if = "Option::is_none")]
    sensor_data: Option<Vec<SensorData>>,
}

fn send_message(message: &Message) -> Result<(), Box<dyn Error>> {
    let agent = ureq::Agent::new();
    match agent
        .post("http://localhost:8080")
        .set("Content-Type", "application/json")
        .send_json(serde_json::to_value(message)?)
    {
        Ok(_) => {
            println!("Message sent successfully");
        }
        Err(ureq::Error::Status(code, response)) => {
            eprintln!(
                "Failed to send message. Code: {}, Status: {}",
                code,
                response.status()
            );
        }
        Err(_) => {
            eprintln!("Failed to send message without getting a response");
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let matches = App::new("device")
        .version("0.1.0")
        .arg(
            Arg::with_name("file")
                .short("f")
                .long("file")
                .takes_value(true)
                .help("Path to the NDJSON file"),
        )
        .arg(
            Arg::with_name("interval")
                .short("i")
                .long("interval")
                .takes_value(true)
                .help("Timing interval between messages (in seconds)"),
        )
        .get_matches();

    let file_path = matches.value_of("file").expect("File path is required");
    let interval = matches
        .value_of("interval")
        .unwrap_or("1")
        .parse::<u64>()
        .expect("Interval must be a number");

    let path = Path::new(file_path);
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        let message: Result<Message, serde_json::Error> = serde_json::from_str(&line);
        match message {
            Ok(m) => send_message(&m)?,
            Err(e) => eprintln!("Error parsing line: {}", e),
        }
        thread::sleep(Duration::from_secs(interval));
    }

    Ok(())
}
