use clap::{App, Arg};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
    thread,
    time::Duration,
    time::SystemTime,
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

fn simulate_messages() -> Result<(), Box<dyn Error>> {
    loop {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();

        let simulated_message = Message {
            timestamp: now as i64,
            device: "SimulatedDevice".to_string(),
            firmware: "1.0-sim".to_string(),
            message_type: MessageType::Log,
            log_message: Some(LogMessage {
                severity: Severity::Info,
                message: "This is a simulated message.".to_string(),
            }),
            sensor_data: None,
        };

        println!("Created simulated message.");

        send_message(&simulated_message)?;

        thread::sleep(Duration::from_secs(1));
    }
}

fn send_messages_from_file(file_path: &str, interval: u64) -> Result<(), Box<dyn Error>> {
    let path = Path::new(&file_path);
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

fn main() -> Result<(), Box<dyn Error>> {
    let matches = App::new("device")
        .version("0.1.0")
        .arg(
            Arg::with_name("file")
                .short("f")
                .long("file")
                .takes_value(true)
                .conflicts_with("simulate")
                .help("Path to the NDJSON file"),
        )
        .arg(
            Arg::with_name("simulate")
                .short("s")
                .long("simulate")
                .takes_value(false)
                .help("Simulate message generation and sending"),
        )
        .arg(
            Arg::with_name("interval")
                .short("i")
                .long("interval")
                .takes_value(true)
                .requires("file")
                .help("Timing interval between messages (in seconds)"),
        )
        .get_matches();

    if matches.is_present("file") {
        let file_path = matches.value_of("file").expect("File path is required");
        let interval = matches
            .value_of("interval")
            .unwrap_or("1")
            .parse::<u64>()
            .expect("Interval must be a number");

        send_messages_from_file(file_path, interval)
    } else {
        simulate_messages()
    }
}
