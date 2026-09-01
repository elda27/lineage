//! 設定の保存。
//!
//! minos（トレイメニュー）も、fullos（設定画面）と同じ `setting_set` mutation を使う。

use anyhow::Result;

use crate::domain::mutation::{MutationOperation, MutationRequest};
use crate::domain::ports::MutationStore;
use crate::domain::settings::Settings;
use crate::domain::shared::{Clock, IdGenerator};
use crate::features::mutation::ApplyMutation;

pub struct SaveSettings<'a> {
    store: &'a dyn MutationStore,
    clock: &'a dyn Clock,
    ids: &'a dyn IdGenerator,
}

impl<'a> SaveSettings<'a> {
    pub fn new(
        store: &'a dyn MutationStore,
        clock: &'a dyn Clock,
        ids: &'a dyn IdGenerator,
    ) -> Self {
        Self { store, clock, ids }
    }

    pub fn execute(&self, workspace_id: &str, settings: Settings) -> Result<()> {
        let apply = ApplyMutation {
            store: self.store,
            clock: self.clock,
        };
        for (key, value) in settings.to_entries() {
            apply.execute(MutationRequest {
                operation_id: self.ids.new_id(),
                workspace_id: workspace_id.into(),
                base_revision: None,
                operation: MutationOperation::SettingSet {
                    key: key.into(),
                    value,
                },
            })?;
        }
        Ok(())
    }
}
