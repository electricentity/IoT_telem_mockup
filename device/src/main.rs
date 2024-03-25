use chrono::{SecondsFormat, Utc};
use clap::{App, Arg};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::error::Error;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
    thread,
};
use tokio::sync::mpsc;
use tokio::time::{self, Duration};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
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
    timestamp: String,
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

async fn send_messages(
    mut messages: Vec<Message>,
    port: u16,
    buffer_size: u64,
) -> Result<(), Box<dyn Error>> {
    let agent = ureq::Agent::new();
    messages.sort_by(|a, b| {
        match (&a.message_type, &b.message_type) {
            // Prioritize Log messages with Severity::Error
            (MessageType::Log, MessageType::Log) => {
                if a.log_message
                    .as_ref()
                    .map_or(false, |log| log.severity == Severity::Error)
                    && b.log_message
                        .as_ref()
                        .map_or(false, |log| log.severity != Severity::Error)
                {
                    std::cmp::Ordering::Less
                } else if a
                    .log_message
                    .as_ref()
                    .map_or(false, |log| log.severity != Severity::Error)
                    && b.log_message
                        .as_ref()
                        .map_or(false, |log| log.severity == Severity::Error)
                {
                    std::cmp::Ordering::Greater
                } else {
                    // If both are errors or both are not errors, consider them equal in this layer
                    std::cmp::Ordering::Equal
                }
            }
            // SensorData comes after Log messages with Severity::Error but before other logs
            (MessageType::Log, MessageType::SensorData) => {
                if a.log_message
                    .as_ref()
                    .map_or(false, |log| log.severity == Severity::Error)
                {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                }
            }
            (MessageType::SensorData, MessageType::Log) => {
                if b.log_message
                    .as_ref()
                    .map_or(false, |log| log.severity == Severity::Error)
                {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Less
                }
            }
            // SensorData messages are considered equal among themselves
            (MessageType::SensorData, MessageType::SensorData) => std::cmp::Ordering::Equal,
        }
    });

    // Trim to down the the buffer size number of messages
    let total_messages = messages.len();
    if total_messages > buffer_size as usize {
        let messages_to_drop = total_messages - buffer_size as usize;
        println!("Dropping {} messages", messages_to_drop);
        messages.truncate(buffer_size as usize);
    }

    for message in messages.into_iter() {
        match agent
            .post(&format!("http://localhost:{}", port))
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
            Err(e) => {
                eprintln!("Failed to send message without getting a response: {:?}", e);
            }
        }
    }

    Ok(())
}

async fn simulate_messages(
    port: u16,
    log_interval_ms: u64,
    sensor_interval_ms: u64,
    write_interval_ms: u64,
    buffer_size: u64,
) {
    let log_message_interval: Duration = Duration::from_millis(log_interval_ms);
    let sensor_data_interval: Duration = Duration::from_millis(sensor_interval_ms);
    let send_interval: Duration = Duration::from_millis(write_interval_ms);

    println!("Creating device");
    // make the channel be larger than the buffer size so we can filter
    // messages in send_messages and pretend we are putting the messages into
    // different quees based on priority
    let (tx, mut rx) = mpsc::channel(2 * buffer_size as usize);
    let device_id = Uuid::new_v4().to_string();

    // Log Message Producer Task
    let tx_clone = tx.clone();
    let device_id_clone = device_id.clone();
    tokio::spawn(async move {
        let mut rng = StdRng::from_entropy(); // Create a random number generator
        loop {
            let random_number = rng.gen_range(1..3);
            let message_type = if random_number == 1 {
                Severity::Error
            } else {
                Severity::Info
            };

            let log_msg = Message {
                timestamp: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
                device: device_id_clone.clone(),
                firmware: "1.0-sim".to_string(),
                message_type: MessageType::Log,
                log_message: Some(LogMessage {
                    severity: message_type,
                    message: "This is a simulated message.".to_string(),
                }),
                sensor_data: None,
            };
            // Send the log message
            if let Err(e) = tx_clone.try_send(log_msg) {
                eprintln!("Failed to put log message into buffer: {:?}", e);
            }
            time::sleep(log_message_interval).await;
        }
    });

    // Sensor Data Producer Task
    tokio::spawn(async move {
        let mut rng = StdRng::from_entropy(); // Create a random number generator
        loop {
            let sensor_msg = Message {
                timestamp: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
                device: device_id.clone(),
                firmware: "1.0-sim".to_string(),
                message_type: MessageType::SensorData,
                log_message: None,
                sensor_data: Some(vec![SensorData {
                    name: "Temp1".to_string(),
                    value: rng.gen_range(1.0..100.0),
                }]),
            };

            tx.send(sensor_msg).await.unwrap();
            time::sleep(sensor_data_interval).await;
        }
    });

    // Central Message Sending Task
    tokio::spawn(async move {
        let mut buffer = VecDeque::new();
        loop {
            // Ensure we have at least one message to send
            if let Some(message) = rx.recv().await {
                buffer.push_back(message);
                // Drain all available messages from the channel
                while let Ok(message) = rx.try_recv() {
                    buffer.push_back(message);
                }
                // Call send_messages with all collected messages
                if let Err(e) = send_messages(buffer.drain(..).collect(), port, buffer_size).await {
                    eprintln!("Failed to send messages: {:?}", e);
                    break;
                }
            } else {
                // Channel is closed
                break;
            }
            time::sleep(send_interval).await;
        }
    })
    .await
    .unwrap();
}

fn send_message_file(message: &Message, port: u16) -> Result<(), Box<dyn Error>> {
    let agent = ureq::Agent::new();
    match agent
        .post(&format!("http://localhost:{}", port))
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

fn send_messages_from_file(
    file_path: &str,
    interval: u64,
    port: u16,
) -> Result<(), Box<dyn Error>> {
    let path = Path::new(&file_path);
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        let message: Result<Message, serde_json::Error> = serde_json::from_str(&line);
        match message {
            Ok(m) => send_message_file(&m, port)?,
            Err(e) => eprintln!("Error parsing line: {}", e),
        }
        thread::sleep(Duration::from_secs(interval));
    }

    Ok(())
}

fn positive_integer_validator(val: String) -> Result<(), String> {
    val.parse::<i64>()
        .map_err(|_| "The value must be an integer.".to_string())
        .and_then(|v| {
            if v > 0 {
                Ok(())
            } else {
                Err("The value must be greater than 0.".to_string())
            }
        })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
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
            Arg::with_name("interval")
                .short("i")
                .long("interval")
                .takes_value(true)
                .requires("file")
                .help("Timing interval between messages (in seconds)"),
        )
        .arg(
            Arg::with_name("simulate")
                .short("s")
                .long("simulate")
                .takes_value(false)
                .help("Simulate message generation and sending"),
        )
        .arg(
            Arg::with_name("log-interval")
                .long("log-interval")
                .takes_value(true)
                .requires("simulate")
                .help("Time between log messages for a single device in ms (default: 500)")
                .validator(positive_integer_validator),
        )
        .arg(
            Arg::with_name("sensor-interval")
                .long("sensor-interval")
                .takes_value(true)
                .requires("simulate")
                .help("Time between sensor messages for a single device in ms (default: 500)")
                .validator(positive_integer_validator),
        )
        .arg(
            Arg::with_name("write-interval")
                .long("write-interval")
                .takes_value(true)
                .requires("simulate")
                .help("Time between sending messages for a single device in ms (default: 500)")
                .validator(positive_integer_validator),
        )
        .arg(
            Arg::with_name("buffer-size")
                .long("buffer-size")
                .takes_value(true)
                .requires("simulate")
                .help("Number of messages a device can hold at a time (default: 3)")
                .validator(positive_integer_validator),
        )
        .arg(
            Arg::with_name("number")
                .short("n")
                .long("number")
                .takes_value(true)
                .requires("simulate")
                .help("Number of devices to simulate (default: 3)")
                .validator(positive_integer_validator),
        )
        .arg(
            Arg::with_name("port")
                .short("p")
                .long("port")
                .takes_value(true)
                .help("The port to try to hit at http://localhost:<PORT> (default: 8080)")
                .validator(positive_integer_validator),
        )
        .get_matches();

    let port: u16 = matches.value_of("port").unwrap_or("8080").parse().unwrap();

    if matches.is_present("file") {
        let file_path = matches.value_of("file").expect("File path is required");
        let interval = matches
            .value_of("interval")
            .unwrap_or("1")
            .parse::<u64>()
            .expect("Interval must be a number");

        send_messages_from_file(file_path, interval, port)
    } else {
        let device_count = matches
            .value_of("number")
            .unwrap_or("3")
            .parse::<u64>()
            .expect("Number of devices must be an integer > 0");

        let buffer_size = matches
            .value_of("buffer-size")
            .unwrap_or("3")
            .parse::<u64>()
            .expect("Buffer size must be an integer > 0");

        let log_interval = matches
            .value_of("log-interval")
            .unwrap_or("500")
            .parse::<u64>()
            .expect("Log interval must be an integer >= 0");

        let sensor_interval = matches
            .value_of("sensor-interval")
            .unwrap_or("500")
            .parse::<u64>()
            .expect("Sensor interval must be an integer >= 0");

        let write_interval = matches
            .value_of("write-interval")
            .unwrap_or("500")
            .parse::<u64>()
            .expect("Write interval must be an integer >= 0");

        let mut simulations = Vec::new();

        for _ in 0..device_count {
            simulations.push(tokio::spawn(async move {
                simulate_messages(
                    port,
                    log_interval,
                    sensor_interval,
                    write_interval,
                    buffer_size,
                )
                .await;
            }));
            // Space things out for the initialization
            thread::sleep(Duration::from_millis(20));
        }

        // Await all simulations to complete (if they ever do)
        for simulation in simulations {
            let _ = simulation.await;
        }
        Ok(())
    }
}
