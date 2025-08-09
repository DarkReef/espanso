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

use std::process::exit;

use crate::{cli::TextViewArgs, icon::IconPaths};
use espanso_modulo::textview::TextViewOptions;

pub fn textview_main(textview_args: TextViewArgs, icon_paths: &IconPaths) -> i32 {
    let data = if textview_args.stdin {
        use std::io::Read;
        let mut buffer = String::new();
        std::io::stdin()
            .read_to_string(&mut buffer)
            .expect("unable to obtain input from stdin");
        buffer
    } else if textview_args.input_file.is_some() {
        std::fs::read_to_string(textview_args.input_file.unwrap())
            .expect("unable to read input file")
    } else {
        println!("Nor --stdin nor `input_file` was passed.");
        exit(1)
    };

    espanso_modulo::textview::show(TextViewOptions {
        window_icon_path: icon_paths
            .wizard_icon
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
        title: textview_args.title.to_string(),
        content: data,
    });

    0
}
