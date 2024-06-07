use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{BufReader};
use std::io::BufRead;
use std::io::{self, Write};


struct LocationData {
    min: i32,
    max: i32,
    sum: i32,
    count: i32,
}


fn main() {
    // Parse the args and get the filename
    let args: Vec<_> = env::args().collect();
    if args.len() != 2 {
        println!("baseline accepts exactly one arg.");
    }
    let file_name = &args[1];

    // Open the file
    let file = File::open(file_name)
        .expect("Should have been able to read the file.");
    let reader = BufReader::new(file);

    // Parse the content
    let mut location_map: HashMap<String, LocationData> = HashMap::new();
    for line in reader.lines() {
        let line = line
            .expect("Should have been a line.");

        let parts: Vec<&str> = line.split(";").collect();

        let location_name = parts[0].to_string();

        // Assume temperature is positive.
        let mut sign = 1;
        let mut temperature_string = parts[1];
        // But if not, set the sign accordingly and remove the minus from the start of the string.
        // We need to do this as we need the absolute temperature value which we then can negate.
        // In case of negative value greater than -1, we start with a -0, which will be parsed to 0.
        // But in case of values lower or equal to -1, we will parse it to a negative integer.
        // Hence we need to remove the sign, in order to always get the absolute value and then negate it afterwards.
        if temperature_string.starts_with('-') {
            sign = -1;
            temperature_string = &temperature_string[1..];
        }

        let temperature_parts: Vec<&str> = temperature_string.split('.').collect();
        let temperature: i32 = (temperature_parts[0].parse::<i32>().unwrap() * 10 + temperature_parts[1].parse::<i32>().unwrap()) * sign;

        if !location_map.contains_key(&location_name) {
            location_map.insert(
                location_name,
                LocationData {
                    min: temperature,
                    max: temperature,
                    sum: temperature,
                    count: 1,
                },
            );
            continue;
        }

        let location: &mut LocationData = location_map.get_mut(&location_name).unwrap();

        if temperature < location.min {
            location.min = temperature;
        }

        if temperature > location.max {
            location.max = temperature;
        }

        location.sum += temperature;
        location.count += 1;
    }

    // Get a sorted list of location names
    let mut sorted_locations: Vec<(&String, &LocationData)> = location_map.iter().collect();
    sorted_locations.sort_by(|a, b| a.0.cmp(b.0));

    // Print everything
    let mut separator = "";
    print!("{}", '{');
    for (location_name, location_data) in &sorted_locations {

        // Get the average and make sure to round mathematically.
        let mut avg = location_data.sum * 10 / location_data.count;
        if avg.abs() % 10 >= 5 {
            avg += 10;
        }
        avg = avg / 10;

        print!(
            "{separator}{location_name}={}{}.{}/{}{}.{}/{}{}.{}",
            if location_data.min < 0 { "-" } else { "" },
            location_data.min.abs() / 10,
            location_data.min.abs() % 10,

            if avg < 0 { "-" } else { "" },
            avg.abs() / 10,
            avg.abs() % 10,

            if location_data.max < 0 { "-" } else { "" },
            location_data.max.abs() / 10,
            location_data.max.abs() % 10,
        );
        separator = ", ";
    }
    println!("{}", '}');

    io::stdout().flush().unwrap();
}
