use std::time::{Duration, Instant};
use sysinfo::{PidExt, ProcessExt, System, SystemExt};

const REFRESH_INTERVAL: Duration = Duration::from_secs(1);

pub struct RuntimeMonitor {
    system: System,
    running: bool,
    process_ids: Vec<u32>,
    last_refresh: Option<Instant>,
    last_change: Instant,
}

impl RuntimeMonitor {
    pub fn new() -> Self {
        Self {
            system: System::new(),
            running: false,
            process_ids: Vec::new(),
            last_refresh: None,
            last_change: Instant::now(),
        }
    }

    pub fn update(&mut self, context: &eframe::egui::Context) {
        context.request_repaint_after(REFRESH_INTERVAL);
        if self
            .last_refresh
            .is_some_and(|updated| updated.elapsed() < REFRESH_INTERVAL)
        {
            return;
        }

        self.system.refresh_processes();
        let mut process_ids = self
            .system
            .processes()
            .iter()
            .filter_map(|(pid, process)| {
                is_respanso_process(process.name()).then(|| pid.as_u32())
            })
            .collect::<Vec<_>>();
        process_ids.sort_unstable();

        let running = !process_ids.is_empty();
        if running != self.running || process_ids != self.process_ids {
            self.last_change = Instant::now();
        }
        self.running = running;
        self.process_ids = process_ids;
        self.last_refresh = Some(Instant::now());
    }

    pub fn running(&self) -> bool {
        self.running
    }

    pub fn process_ids(&self) -> &[u32] {
        &self.process_ids
    }

    pub fn seconds_since_change(&self) -> u64 {
        self.last_change.elapsed().as_secs()
    }
}

fn is_respanso_process(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "espansod" | "espansod.exe" | "respansod" | "respansod.exe" | "espanso" | "espanso.exe"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_daemon_and_cli_process_names() {
        assert!(is_respanso_process("espansod.exe"));
        assert!(is_respanso_process("rEspansod.exe"));
        assert!(is_respanso_process("ESPANSO"));
        assert!(!is_respanso_process("rEspanso Match Studio.exe"));
    }
}
