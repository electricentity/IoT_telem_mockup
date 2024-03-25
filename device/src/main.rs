use chrono::{SecondsFormat, Utc};
use clap::{App, Arg};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
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

const LOG_MESSAGE_INTERVAL: Duration = Duration::from_millis(500000);
const SENSOR_DATA_INTERVAL: Duration = Duration::from_millis(500);
const SEND_INTERVAL: Duration = Duration::from_millis(3000);

async fn send_message(message: &Message, port: u16) -> Result<(), Box<dyn Error>> {
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
        Err(e) => {
            eprintln!("Failed to send message without getting a response: {:?}", e);
        }
    }

    Ok(())
}

async fn simulate_messages(port: u16) {
    println!("Creating device");
    let (tx, mut rx) = mpsc::channel(3);
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
            if let Err(e) = tx_clone.send(log_msg).await {
                eprintln!("Failed to send log message: {:?}", e);
                break;
            }
            time::sleep(LOG_MESSAGE_INTERVAL).await;
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
            time::sleep(SENSOR_DATA_INTERVAL).await;
        }
    });

    // Central Message Sending Task
    tokio::spawn(async move {
        loop {
            if let Some(message) = rx.recv().await {
                // Send the message
                send_message(&message, port).await.unwrap();
            }
            time::sleep(SEND_INTERVAL).await;
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
            Arg::with_name("number")
                .short("n")
                .long("number")
                .takes_value(true)
                .requires("simulate")
                .help("Number of devices to simulate (default: 3)"),
        )
        .arg(
            Arg::with_name("port")
                .short("p")
                .long("port")
                .takes_value(true)
                .help("The port to try to hit at http://localhost:<PORT> (default: 8080)")
                .validator(|v| match v.parse::<u16>() {
                    Ok(port) => {
                        if port > 0 {
                            Ok(())
                        } else {
                            Err("Port number must be greater than 0.".to_string())
                        }
                    }
                    Err(_) => Err("Port number must be a valid integer.".to_string()),
                }),
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
        let mut simulations = Vec::new();

        for _ in 0..device_count {
            simulations.push(tokio::spawn(async move {
                simulate_messages(port).await;
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
