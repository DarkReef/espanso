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

use crate::{cli::InstallArgs, path::Paths};
use anyhow::{bail, Context, Result};
use espanso_package::{PackageSpecifier, ProviderOptions, SaveOptions};

use crate::{error_eprintln, info_println};

pub fn install_package(paths: &Paths, install_args: InstallArgs) -> Result<()> {
    let package_name = install_args.package_name;
    let version = match install_args.version {
        Some(version) => version,
        None => String::from("latest"),
    };

    let force = install_args.force;
    let refresh_index = install_args.refresh_index;
    let external = install_args.external;

    info_println!("installing package: {package_name} - version: {}", version);

    let git_repo = install_args.git_repo;
    let (package_specifier, requires_external) = if git_repo.is_some() {
        let git_branch = install_args.git_branch;
        let use_native_git = install_args.use_native_git;
        (
            PackageSpecifier {
                name: package_name.to_string(),
                version: Some(version),
                git_repo_url: git_repo,
                git_branch: git_branch.map(String::from),
                use_native_git,
            },
            true,
        )
    } else {
        // Install from the hub
        (
            PackageSpecifier {
                name: package_name.to_string(),
                version: Some(version),
                ..Default::default()
            },
            false,
        )
    };

    if requires_external && !external {
        error_eprintln!("Error: the requested package is hosted on an external repository");
        error_eprintln!("and its contents may not have been verified by the espanso team.");
        error_eprintln!("");
        error_eprintln!(
            "For security reasons, espanso blocks packages that are not verified by default."
        );
        error_eprintln!(
            "If you want to install the package anyway, you can proceed with the installation"
        );
        error_eprintln!("by passing the '--external' flag, but please do it only if you trust the");
        error_eprintln!("source or you verified the contents of the package yourself.");
        error_eprintln!("");

        bail!("installing from external repository without --external flag");
    }

    let package_provider = espanso_package::get_provider(
        &package_specifier,
        &paths.runtime,
        &ProviderOptions {
            force_index_update: refresh_index,
        },
    )
    .context("unable to obtain compatible package provider")?;

    info_println!("using package provider: {}", package_provider.name());

    let package = package_provider.download(&package_specifier)?;

    info_println!(
        "found package: {} - version: {}",
        package.name(),
        package.version()
    );

    let archiver =
        espanso_package::get_archiver(&paths.packages).context("unable to get package archiver")?;

    archiver
        .save(
            &*package,
            &package_specifier,
            &SaveOptions {
                overwrite_existing: force,
            },
        )
        .context("unable to save package")?;

    info_println!("package installed!");

    Ok(())
}
