use clap::{App, Arg};
use serde::Deserialize;
use serde_json::Error;
use std::{fs::File, io::{self, BufRead, BufReader}, path::Path, thread, time::Duration};

#[derive(Deserialize, Debug)]
enum Severity {
    Debug,
    Info,
    Warning,
    Error,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
enum MessageType {
    Log,
    SensorData,
}

#[derive(Deserialize, Debug)]
struct LogMessage {
    severity: Severity,
    message: String,
}

#[derive(Deserialize, Debug)]
struct SensorData {
    name: String,
    value: f32,
}


#[derive(Deserialize, Debug)]
struct Message {
    timestamp: i64,
    device: String,
    firmware: String,
    message_type: MessageType,
    #[serde(default)] // Makes the field optional during deserialization
    log_message: Option<LogMessage>,
    #[serde(default)] // Makes the field optional during deserialization
    sensor_data: Option<Vec<SensorData>>,
}

fn main() -> Result<(), io::Error> {
    let matches = App::new("device")
        .version("0.1.0")
        .arg(Arg::with_name("file")
             .short("f")
             .long("file")
             .takes_value(true)
             .help("Path to the NDJSON file"))
        .arg(Arg::with_name("interval")
             .short("i")
             .long("interval")
             .takes_value(true)
             .help("Timing interval between messages (in seconds)"))
        .get_matches();

    let file_path = matches.value_of("file").expect("File path is required");
    let interval = matches.value_of("interval")
                           .unwrap_or("1")
                           .parse::<u64>()
                           .expect("Interval must be a number");

    let path = Path::new(file_path);
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        let message: Result<Message, Error> = serde_json::from_str(&line);
        match message {
            Ok(m) => println!("{:?}", m), // Adjust based on your Message struct.
            Err(e) => eprintln!("Error parsing line: {}", e),
        }
        thread::sleep(Duration::from_secs(interval));
    }

    Ok(())
}
