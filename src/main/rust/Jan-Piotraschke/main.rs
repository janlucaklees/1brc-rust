use std::time::Instant;
use std::path::PathBuf;
use std::fs::File;
use std::io::{self, Read};
use std::collections::HashMap;

#[derive(Debug)]
struct TempAnalysis {
    counter: i32,
    sum: f64,
    min: f64,
    max: f64,
}

impl TempAnalysis {
    fn new(value: f64) -> Self {
        TempAnalysis {
            counter: 1,
            sum: value,
            min: value,
            max: value,
        }
    }

    fn update(&mut self, value: f64) {
        self.counter += 1;
        self.sum += value;
        if value < self.min {
            self.min = value;
        }
        if value > self.max {
            self.max = value;
        }
    }

    fn mean(&self) -> f64 {
        self.sum / self.counter as f64
    }
}


// function to read file to string with proper error handling
fn read_file_to_string(path: &PathBuf) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(content)
}

fn main() {
    // Start the timer
    let start = Instant::now();

    // get the command line arguments
    let args: Vec<String> = std::env::args().collect();

    // check if the argument is provided
    if args.len() < 2 {
        eprintln!("Usage: {} <file_path>", args[0]);
        std::process::exit(1);
    }

    // read the file from the string
    let path = PathBuf::from(&args[1]);
    let content = read_file_to_string(&path).expect("Could not read the file");

    // create a HashMap to store the analysis data
    let mut analysis_map: HashMap<String, TempAnalysis> = HashMap::new();

    // process each line of the file
    for line in content.lines() {
        let parts: Vec<&str> = line.split(';').collect();
        if parts.len() != 2 {
            eprintln!("Invalid line format: {}", line);
            continue;
        }
        
        let name = parts[0].to_string();
        let value: f64 = match parts[1].parse() {
            Ok(v) => v,
            Err(_) => {
                eprintln!("Invalid number format: {}", parts[1]);
                continue;
            }
        };

        analysis_map.entry(name.clone())
            .and_modify(|e| e.update(value))
            .or_insert_with(|| TempAnalysis::new(value));
    }

    // create a vector of tuples from the HashMap and sort it by the station name
    let mut sorted_analysis: Vec<(&String, &TempAnalysis)> = analysis_map.iter().collect();
    sorted_analysis.sort_by(|a, b| a.0.cmp(b.0));

    // prepare and print the formatted output
    let formatted_output: Vec<String> = sorted_analysis.iter().map(|(name, analysis)| {
        format!(
            "{}={:.1}/{:.1}/{:.1}", 
            name, 
            analysis.min, 
            analysis.mean(), 
            analysis.max
        )
    }).collect();

    println!("{{{}}}", formatted_output.join(", "));

    // Stop the timer and print the duration
    let duration = start.elapsed();
    println!("Time elapsed: {:?}", duration);
}
