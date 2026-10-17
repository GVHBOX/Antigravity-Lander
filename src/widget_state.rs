use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WidgetMode {
    Smart,
    Always,
}

impl Default for WidgetMode {
    fn default() -> Self {
        Self::Smart
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WidgetConfig {
    pub enabled: bool,
    pub mode: WidgetMode,
    pub notify_on_done: bool,
    pub pos_x: i32,
    pub pos_y: i32,
    pub edge_snap: bool,
}

impl Default for WidgetConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: WidgetMode::Smart,
            notify_on_done: true,
            pos_x: 1680,
            pos_y: 300,
            edge_snap: true,
        }
    }
}

impl WidgetConfig {
    pub fn config_path() -> PathBuf {
        PathBuf::from("data").join("floating_widget_settings.json")
    }

    pub fn load_or_default() -> Self {
        let path = Self::config_path();
        Self::load_from_path(&path).unwrap_or_default()
    }

    pub fn load_from_path(path: &Path) -> Option<Self> {
        let content = fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        self.save_to_path(&path)
    }

    pub fn save_to_path(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SessionTransition {
    Started {
        slot: u8,
    },
    Finished {
        slot: u8,
        duration_secs: f32,
        total_tokens: u64,
        avg_tps: f32,
    },
}

#[derive(Clone, Debug)]
pub struct SlotRuntimeRecord {
    pub is_running: bool,
    pub start_time: Option<Instant>,
    pub initial_tokens: u64,
    pub last_tokens: u64,
    pub last_tps: f32,
}

impl Default for SlotRuntimeRecord {
    fn default() -> Self {
        Self {
            is_running: false,
            start_time: None,
            initial_tokens: 0,
            last_tokens: 0,
            last_tps: 0.0,
        }
    }
}

pub struct StateTransitionDetector {
    slots: [SlotRuntimeRecord; 5],
}

impl Default for StateTransitionDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl StateTransitionDetector {
    pub fn new() -> Self {
        Self {
            slots: Default::default(),
        }
    }

    pub fn update(
        &mut self,
        slot_inputs: &[(u8, bool, u64, f32)],
    ) -> Vec<SessionTransition> {
        let mut transitions = Vec::new();
        let now = Instant::now();

        for &(slot, running, tokens, tps) in slot_inputs {
            let idx = slot as usize;
            if idx >= self.slots.len() {
                continue;
            }

            let prev_running = self.slots[idx].is_running;
            if !prev_running && running {
                self.slots[idx].is_running = true;
                self.slots[idx].start_time = Some(now);
                self.slots[idx].initial_tokens = tokens;
                self.slots[idx].last_tokens = tokens;
                self.slots[idx].last_tps = tps;
                transitions.push(SessionTransition::Started { slot });
            } else if prev_running && !running {
                let duration_secs = self.slots[idx]
                    .start_time
                    .map(|st| now.duration_since(st).as_secs_f32())
                    .unwrap_or(0.0);
                let diff_tokens = if tokens >= self.slots[idx].initial_tokens {
                    tokens - self.slots[idx].initial_tokens
                } else {
                    tokens
                };
                let avg_tps = if duration_secs > 0.1 {
                    diff_tokens as f32 / duration_secs
                } else {
                    self.slots[idx].last_tps
                };

                self.slots[idx].is_running = false;
                self.slots[idx].start_time = None;
                self.slots[idx].initial_tokens = 0;
                self.slots[idx].last_tokens = tokens;
                self.slots[idx].last_tps = 0.0;

                transitions.push(SessionTransition::Finished {
                    slot,
                    duration_secs,
                    total_tokens: diff_tokens,
                    avg_tps,
                });
            } else if running {
                self.slots[idx].last_tokens = tokens;
                self.slots[idx].last_tps = tps;
            }
        }

        transitions
    }

    pub fn has_any_running(&self) -> bool {
        self.slots.iter().any(|s| s.is_running)
    }

    pub fn get_slot_state(&self, slot: u8) -> Option<&SlotRuntimeRecord> {
        self.slots.get(slot as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_widget_config_default_and_serde() {
        let config = WidgetConfig::default();
        assert!(config.enabled);
        assert_eq!(config.mode, WidgetMode::Smart);
        assert!(config.notify_on_done);

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: WidgetConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_transition_detector_lifecycle() {
        let mut detector = StateTransitionDetector::new();
        assert!(!detector.has_any_running());

        let t1 = detector.update(&[(1, true, 100, 50.0)]);
        assert_eq!(t1.len(), 1);
        assert_eq!(t1[0], SessionTransition::Started { slot: 1 });
        assert!(detector.has_any_running());

        let t2 = detector.update(&[(1, true, 200, 60.0)]);
        assert!(t2.is_empty());
        assert!(detector.has_any_running());

        let t3 = detector.update(&[(1, false, 350, 0.0)]);
        assert_eq!(t3.len(), 1);
        match t3[0] {
            SessionTransition::Finished {
                slot,
                total_tokens,
                ..
            } => {
                assert_eq!(slot, 1);
                assert_eq!(total_tokens, 250);
            }
            _ => panic!("expected Finished transition"),
        }
        assert!(!detector.has_any_running());
    }
}
