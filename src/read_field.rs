#![allow(clippy::type_complexity)]

use std::collections::HashMap;
use std::fs;

pub fn rank_data(entries: &HashMap<String, i32>) -> Vec<(String, i32)> {
    let mut ranked: Vec<(String, i32)> = entries
        .iter()
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1));

    ranked
}

pub fn read_field(field: &i32, dir_path: &str) -> HashMap<String, i32> {
    let error_message = format!("something went wrong reading directory {}", dir_path);
    let file_names = fs::read_dir(dir_path).expect(&error_message);

    let mut field_data = HashMap::new();

    for path in file_names {
        let file_name = path.expect(&error_message).path();
        if file_name.is_dir() {
            let subdir_field_data = read_field(field, file_name.to_str().unwrap());
            field_data.extend(subdir_field_data);
        } else {
            let error_message = format!(
                "something went wrong reading file {}",
                file_name.display()
            );
            let contents = fs::read_to_string(file_name).expect(&error_message);
            let lines = contents.lines();

            for line in lines {
                let words: Vec<&str> = line.split(',').collect();
                let entry = words[*field as usize].parse().unwrap();
                let count = field_data.entry(entry).or_insert(0);
                *count += 1;
            }
        }
    }

    field_data
}
