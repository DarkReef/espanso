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

#[cfg(feature = "modulo")]
use crate::{cli::ModuloArgs, path::Paths};

#[cfg(feature = "modulo")]
mod form;
#[cfg(feature = "modulo")]
mod search;
#[cfg(feature = "modulo")]
mod textview;
#[cfg(feature = "modulo")]
mod troubleshoot;
#[cfg(feature = "modulo")]
mod welcome;

#[cfg(feature = "modulo")]
pub fn modulo_main(cli_args: ModuloArgs, paths: Paths) -> i32 {
    let icon_paths =
        crate::icon::load_icon_paths(&paths.runtime).expect("unable to load icon paths");

    match cli_args {
        ModuloArgs::Form => form::form_main(ModuloArgs, &icon_paths)
        ModuloArgs::Search => todo!(),
        ModuloArgs::TextView => todo!(),
        ModuloArgs::Troubleshoot => todo!(),
        ModuloArgs::Welcome => todo!(),
    }


    if let Some(matches) = cli_args.subcommand_matches("search") {
        return search::search_main(matches, &icon_paths);
    }

    if let Some(matches) = cli_args.subcommand_matches("welcome") {
        return welcome::welcome_main(matches, &paths, &icon_paths);
    }

    if let Some(matches) = cli_args.subcommand_matches("textview") {
        return textview::textview_main(matches, &icon_paths);
    }

    if cli_args.subcommand_matches("troubleshoot").is_some() {
        return troubleshoot::troubleshoot_main(&paths, &icon_paths);
    }

    0
}

#[cfg(not(feature = "modulo"))]
pub fn modulo_main(_: ModuloArgs, _: Paths) -> i32 {
    panic!("this version of espanso was not compiled with 'modulo' support, please obtain a version that does or recompile it with the 'modulo' feature flag");
}
