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

use crate::{cli::MatchArgs, config::ConfigLoadResult, path::Paths};

mod exec;
mod list;

pub fn match_main(match_args: MatchArgs, paths: Paths, config_result: ConfigLoadResult) -> i32 {
    match match_args {
        MatchArgs::Exec { trigger } => {
            if let Err(err) = exec::exec_main(trigger, &paths) {
                eprintln!("unable to exec match: {err:?}");
                return 1;
            }
        }
        MatchArgs::List(match_list_command) => {
            if let Err(err) = list::list_main(
                &match_list_command,
                config_result.config_store,
                config_result.match_store,
            ) {
                eprintln!("unable to list matches: {err:?}");
                return 1;
            }
        }
    }
    0
}
