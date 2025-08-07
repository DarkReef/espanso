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

use crate::{cli::WorkaroundArgs, error_eprintln};

#[cfg(target_os = "macos")]
mod secure_input;

pub fn workaround_main(workaround_arg: WorkaroundArgs) -> i32 {
    match workaround_arg {
        WorkaroundArgs::SecureInput => {
            #[cfg(target_os = "macos")]
            {
                if let Err(err) = secure_input::run_secure_input_workaround() {
                    error_eprintln!("secure-input workaround reported error: {}", err);
                    return 1;
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                error_eprintln!("secure-input workaround is only available on macOS");
            }
        }
    }

    0
}
