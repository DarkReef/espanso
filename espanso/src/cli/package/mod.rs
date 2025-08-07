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

use crate::{
    cli::PackageArgs,
    error_eprintln,
    exit_code::{
        configure_custom_panic_hook, PACKAGE_INSTALL_FAILED, PACKAGE_LIST_FAILED, PACKAGE_SUCCESS,
        PACKAGE_UNEXPECTED_FAILURE, PACKAGE_UNINSTALL_FAILED, PACKAGE_UPDATE_FAILED,
        PACKAGE_UPDATE_PARTIAL_FAILURE,
    },
    path::Paths,
};

mod install;
mod list;
mod uninstall;
mod update;

pub fn package_main(cli_args: PackageArgs, paths: Paths) -> i32 {
    configure_custom_panic_hook(PACKAGE_UNEXPECTED_FAILURE);

    match cli_args {
        PackageArgs::Install(install_args) => {
            if let Err(err) = install::install_package(&paths, install_args) {
                error_eprintln!("unable to install package: {:?}", err);
                return PACKAGE_INSTALL_FAILED;
            }
        }
        PackageArgs::Uninstall(uninstall_args) => {
            if let Err(err) = uninstall::uninstall_package(&paths, uninstall_args) {
                error_eprintln!("unable to uninstall package: {:?}", err);
                return PACKAGE_UNINSTALL_FAILED;
            }
        }
        PackageArgs::List => {
            if let Err(err) = list::list_packages(&paths) {
                error_eprintln!("unable to list packages: {:?}", err);
                return PACKAGE_LIST_FAILED;
            }
        }
        PackageArgs::Update {
            package_name_or_all,
        } => match update::update_package(&paths, package_name_or_all) {
            Ok(update::UpdateResults::PartialFailure) => {
                error_eprintln!("some packages were updated, but not all of them. Check the previous log for more information");
                return PACKAGE_UPDATE_PARTIAL_FAILURE;
            }
            Err(err) => {
                error_eprintln!("unable to update package: {:?}", err);
                return PACKAGE_UPDATE_FAILED;
            }
            _ => {}
        },
    }

    0
}
