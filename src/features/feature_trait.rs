use crate::StatusBar;
use std::sync::Arc;
use tokio::sync::{Notify, RwLock};

#[async_trait::async_trait]
pub trait FeatureTrait {
    /// Seed a worker with its shared output slots.
    fn new(status_bar: Arc<StatusBar>) -> Self
    where
        Self: Sized;

    /// Publish samples until the worker stops.
    async fn pull(&mut self);
}

#[derive(Default)]
pub(super) struct FeatureState {
    failed: bool,
}

pub(super) struct FeatureUpdate {
    output: String,
    event: Option<String>,
}

impl FeatureState {
    /// Convert one sample into output and a transition-only event.
    pub(super) fn update(
        &mut self,
        feature: &str,
        sample: Result<String, String>,
    ) -> FeatureUpdate {
        match sample {
            Ok(output) => {
                let event = if self.failed {
                    Some(format!("Feature {feature} recovered"))
                } else {
                    None
                };

                self.failed = false;

                FeatureUpdate { output, event }
            }
            Err(err) => {
                let event = if self.failed {
                    None
                } else {
                    Some(format!("Feature {feature} failed: {err}"))
                };

                self.failed = true;

                FeatureUpdate {
                    output: String::new(),
                    event,
                }
            }
        }
    }
}

/// Publish one sample and write only a health transition to stderr.
pub(super) async fn _publish_update(
    update: FeatureUpdate,
    slot: &RwLock<String>,
    redraw: &Notify,
) {
    if let Some(event) = update.event {
        eprintln!("{event}");
    }

    *slot.write().await = update.output;
    redraw.notify_one();
}

#[cfg(test)]
mod tests {
    use super::FeatureState;

    /// Emit one contextual event on the first failed sample.
    #[test]
    fn report_first_failure() {
        let mut state = FeatureState::default();
        let update = state.update(
            "gpu",
            Err("Failed to run nvidia-smi: No such file or directory".to_string()),
        );

        assert_eq!(update.output, "");
        assert_eq!(
            update.event,
            Some(
                "Feature gpu failed: Failed to run nvidia-smi: No such file or directory"
                    .to_string(),
            ),
        );
    }

    /// Suppress repeated reports while a feature stays unavailable.
    #[test]
    fn suppress_repeated_failure() {
        let mut state = FeatureState::default();

        state.update("cpu", Err("first read failed".to_string()));
        let update = state.update("cpu", Err("second read failed".to_string()));

        assert_eq!(update.output, "");
        assert_eq!(update.event, None);
    }

    /// Republish output and report the first successful recovery.
    #[test]
    fn report_recovery() {
        let mut state = FeatureState::default();

        state.update("gpu", Err("nvidia-smi failed".to_string()));
        let update = state.update("gpu", Ok("gpu 42%".to_string()));

        assert_eq!(update.output, "gpu 42%");
        assert_eq!(update.event, Some("Feature gpu recovered".to_string()));
    }

    /// Apply the same transition policy to every fallible sampler.
    #[test]
    fn identify_each_sampler_failure() {
        for feature in ["cpu", "ram", "gpu", "traffic"] {
            let mut state = FeatureState::default();
            let update = state.update(feature, Err("injected read failure".to_string()));

            assert_eq!(
                update.event,
                Some(format!("Feature {feature} failed: injected read failure")),
            );
        }
    }
}
