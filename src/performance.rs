use chardetng::EncodingDetector;
use charset_normalizer_rs::consts::CHARDET_CORRESPONDENCE;
use charset_normalizer_rs::entity::{PerformanceArgs, PerformanceResult};
use charset_normalizer_rs::from_bytes;
use charset_normalizer_rs::utils::{get_large_test_datasets, round_float};
use clap::Parser;
use encoding::label::encoding_from_whatwg_label;
use log::trace;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::process;
use std::time::{Duration, Instant};

// Calculate percentile
fn calc_percentile(results: &Vec<PerformanceResult>, percentile: f64) -> Duration {
    let mut sorted_data: Vec<Duration> = results.iter().map(|r| r.duration).collect();
    sorted_data.sort();
    let index = ((percentile / 100.0) * sorted_data.len() as f64) as usize;
    sorted_data[index]
}

// Calculate mean duration
fn calc_stat(results: &Vec<PerformanceResult>) -> (Duration, Duration, f32) {
    let durations: Vec<Duration> = results.iter().map(|r| r.duration).collect();
    if durations.is_empty() {
        // Handle the case where the input vector is empty (avoid division by zero)
        (Duration::new(0, 0), Duration::new(0, 0), 0.0)
    } else {
        // Calculate the total duration by summing all the durations in the vector
        let total_duration: Duration = durations.iter().sum();

        // Divide the total duration by the number of durations to get the mean
        let num_durations = durations.len() as u32;

        // Accuracy
        let accuracy = round_float(
            100.0 * results.iter().filter(|r| r.correct).count() as f32 / num_durations as f32,
            1,
        );

        (total_duration, total_duration / num_durations, accuracy)
    }
}

// Performance comparison
fn performance_compare(args: &PerformanceArgs) -> i32 {
    // read datasets from /src/tests/data/largesets
    let datasets = get_large_test_datasets();
    if datasets.is_err() {
        println!("{}", datasets.unwrap_err());
        process::exit(1);
    }
    let datasets = datasets.unwrap();
    let nof_files = datasets.len();
    println!("Found {} datasets for performance tests", nof_files);

    // tested functions
    let mut performance_results: HashMap<&str, Vec<PerformanceResult>> = HashMap::new();
    let mut tested_functions: HashMap<&str, Box<dyn Fn(&Vec<u8>) -> String>> = HashMap::new();

    /////////////////////////////////////////////////////////////////
    // Tested functions (libraries)
    /////////////////////////////////////////////////////////////////

    // charset-normalizer-rs
    tested_functions.insert(
        "charset-normalizer-rs",
        Box::new(|bytes: &Vec<u8>| {
            if let Some(gb) = from_bytes(bytes, None).get_best() {
                gb.encoding().to_string()
            } else {
                String::from("None")
            }
        }),
    );

    // chardet
    tested_functions.insert(
        "chardet",
        Box::new(|bytes: &Vec<u8>| {
            let detected = &chardet::detect(bytes).0.to_ascii_lowercase();
            let alternative = CHARDET_CORRESPONDENCE.get(&detected.as_str());
            if let Some(r) = encoding_from_whatwg_label(&detected) {
                r.whatwg_name()
                    .unwrap_or(alternative.unwrap_or(&r.name()))
                    .to_string()
            } else {
                String::from("None")
            }
        }),
    );

    // chardetng
    tested_functions.insert(
        "chardetng",
        Box::new(|bytes: &Vec<u8>| {
            let mut ed = EncodingDetector::new();
            ed.feed(bytes, true);
            let found = ed.guess(None, true).name();
            found.to_ascii_lowercase().to_string()
        }),
    );

    // start tests
    for (filename, correct_encodings) in &datasets {
        println!("{}", filename);

        // read file contents to buffer
        let mut file = File::open(filename).expect(&format!("Error opening file {}", filename));
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .expect(&format!("Error reading from file {}", filename));

        // multiply buffer
        buffer = buffer.repeat(args.size_increase as usize);

        // traverse tested functions
        for (&name, &ref foo) in &tested_functions {
            if !performance_results.contains_key(name) {
                performance_results.insert(name, vec![]);
            }
            let duration = Instant::now();
            let guessed_encoding = foo(&buffer);
            let duration = duration.elapsed();
            performance_results
                .get_mut(name)
                .unwrap()
                .push(PerformanceResult {
                    duration,
                    correct: correct_encodings.contains(&guessed_encoding.to_string()),
                });
            println!("  --> {}: {:?}", name, duration,);

            if !correct_encodings.contains(&guessed_encoding.to_string()) {
                trace!(
                    "{} WRONG DETECTION: {} not in {:?}\nSee {}",
                    name,
                    guessed_encoding,
                    correct_encodings,
                    filename,
                );
            }
        }
    }

    // Statistics
    let mut our_accuracy = 0.0;
    for (&name, _) in &tested_functions {
        if let Some(results) = performance_results.get(name) {
            let (total_duration, mean_duration, accuracy) = calc_stat(results);
            if name == "charset-normalizer-rs" {
                our_accuracy = accuracy;
            }
            println!("\n------------------------------");
            println!("--> {} Conclusions", name);
            println!("   --> Accuracy: {:?}%", accuracy);
            println!("   --> Total time: {:?}", total_duration);
            println!("   --> Avg time: {:?}", mean_duration);
            for p in [50.0, 95.0, 99.0] {
                println!("   --> {}th: {:?}", p, calc_percentile(results, p));
            }
        }
    }

    // Correct exit code, if charset-normalizer-rs accuracy lower than 95%
    if our_accuracy < 95.0 {
        1
    } else {
        0
    }
}

// Main function
pub fn main() {
    let args = PerformanceArgs::parse();
    process::exit(performance_compare(&args));
}