/*
 * This file is part of espanso.
 *
 * Copyright (C) 2019-2021 Federico Terzi
 *
 * espanso is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * espanso is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with espanso.  If not, see <https://www.gnu.org/licenses/>.
 */

use std::io::BufRead;
use std::{fs::File, io::BufReader};

use crate::path::Paths;

pub fn log_main(paths: Option<Paths>) -> i32 {
    let paths = paths.expect("missing paths argument");
    let log_file = paths.runtime.join(crate::LOG_FILE_NAME);

    if !log_file.exists() {
        eprintln!("No log file found.");
        return 2;
    }

    let log_file = File::open(log_file);
    if let Ok(log_file) = log_file {
        let reader = BufReader::new(log_file);
        for line in reader.lines().map_while(Result::ok) {
            println!("{line}");
        }
    } else {
        eprintln!("Error reading log file");
        return 1;
    }

    0
}
