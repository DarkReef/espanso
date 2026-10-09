/*
 * This file is part of espanso.
 *
 * Copyright (C) 2019-2021 Federico Terzi
 *
 * espanso is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 */

use anyhow::Result;
use fs2::FileExt;
use log::{error, warn};
use std::{
    fs::{File, OpenOptions},
    path::Path,
};

pub struct Lock {
    lock_file: File,
}

impl Lock {
    #[allow(dead_code)]
    pub fn release(self) -> Result<()> {
        fs2::FileExt::unlock(&self.lock_file)?;
        Ok(())
    }

    fn acquire(runtime_dir: &Path, name: &str) -> Option<Lock> {
        let lock_file_path = runtime_dir.join(format!("{name}.lock"));
        let lock_file = match OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_file_path)
        {
            Ok(file) => file,
            Err(error) => {
                error!(
                    "unable to open lock file {}: {error}",
                    lock_file_path.display()
                );
                return None;
            }
        };
        if lock_file.try_lock_exclusive().is_ok() {
            Some(Lock { lock_file })
        } else {
            None
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        if let Err(error) = fs2::FileExt::unlock(&self.lock_file) {
            warn!("unable to unlock lock file during shutdown: {error}");
        }
    }
}

pub fn acquire_daemon_lock(runtime_dir: &Path) -> Option<Lock> {
    Lock::acquire(runtime_dir, "espanso-daemon")
}

pub fn acquire_worker_lock(runtime_dir: &Path) -> Option<Lock> {
    Lock::acquire(runtime_dir, "espanso-worker")
}
