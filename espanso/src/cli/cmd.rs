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

use std::path::Path;

use crate::{
    cli::CmdCommand,
    ipc::{create_ipc_client_to_worker, IPCEvent},
    lock::acquire_worker_lock,
    path::Paths,
};

use anyhow::{bail, Result};
use espanso_ipc::IPCClient;

pub fn cmd_main(cmd_args: CmdCommand, paths: Paths) -> i32 {
    let event = match cmd_args {
        CmdCommand::Disable => IPCEvent::DisableRequest,
        CmdCommand::Enable => IPCEvent::EnableRequest,
        CmdCommand::Search => IPCEvent::OpenSearchBar,
        CmdCommand::Toggle => IPCEvent::ToggleRequest,
        // missing `IPCEvent::OpenSearchBar` and `IPCEvent::OpenConfigFolder`
    };

    if let Err(error) = send_event_to_worker(&paths.runtime, event) {
        eprintln!("unable to send command, error: {error:?}");
        return 2;
    }

    0
}

fn send_event_to_worker(runtime_path: &Path, event: IPCEvent) -> Result<()> {
    if acquire_worker_lock(runtime_path).is_some() {
        bail!("Worker process is not running, please start Espanso first.")
    }

    let mut client = create_ipc_client_to_worker(runtime_path)?;
    client.send_async(event)
}
