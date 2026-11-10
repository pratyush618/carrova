//! Pass 3: item-level checks — handlers to run, apps missing or running,
//! free space. Each unresolved problem is a [`Blocker`] unless the user
//! overrode it (then a warning).

use std::collections::{BTreeMap, BTreeSet};

use carrova_recipe::Handler;

use super::Planner;
use crate::plan::{Blocker, PlannedHandler};
use crate::space::SpaceNeed;

impl<'a> Planner<'a> {
    /// Handlers of selected items that can run here: known, and backed by
    /// a matching local recipe item declaring the files they change.
    pub(super) fn handlers(&mut self) -> Vec<PlannedHandler> {
        let mut out = Vec::new();
        for idx in self.selected.clone() {
            let item = &self.manifest.items[idx];
            let Some(name) = item.handler.as_deref() else {
                continue;
            };
            let key = self.keys[idx].clone();
            let Some(handler) = Handler::from_name(name) else {
                self.warn(
                    Some(&key),
                    format!("unknown handler `{name}`: item not applied"),
                );
                continue;
            };
            let Some(recipe_item) = self.local_item(item).filter(|i| i.handler == Some(handler))
            else {
                self.warn(
                    Some(&key),
                    format!(
                        "handler `{name}` not run: no matching recipe item on this machine, so the files it changes cannot be backed up"
                    ),
                );
                continue;
            };
            out.push(PlannedHandler {
                item: key,
                handler,
                backup: recipe_item
                    .paths
                    .iter()
                    .map(|p| (p.root, p.rel.as_str().to_owned()))
                    .collect(),
                import_trust: self.options.import_trust,
            });
        }
        out
    }

    fn selected_recipe_ids(&self, only_requires_closed: bool) -> BTreeSet<&'a str> {
        let manifest = self.manifest;
        self.selected
            .iter()
            .map(|&i| &manifest.items[i])
            .filter(|it| !only_requires_closed || it.flags.requires_closed)
            .filter_map(|it| it.recipe_id.as_deref())
            .collect()
    }

    /// Recipes of selected items whose app is not detected here.
    pub(super) fn missing_apps(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        for id in self.selected_recipe_ids(false) {
            let detected = self
                .recipe(id)
                .is_some_and(|r| carrova_recipe::detect(r, self.platform).detected);
            if !detected {
                out.push(id.to_owned());
                self.warn(
                    None,
                    format!("app `{id}` is not installed here: its data is restored, install the app to use it"),
                );
            }
        }
        out
    }

    /// Running apps that must be closed: blockers unless overridden.
    pub(super) fn running_apps(&mut self) -> Vec<String> {
        let running = self.detect_running();
        for id in &running {
            let name = self.app_name(id);
            if self.options.ignore_requires_closed {
                self.warn(
                    None,
                    format!("override: {name} is running and will be changed while open (requires-closed check ignored)"),
                );
            } else {
                self.blockers.push(Blocker::RunningApp {
                    app: id.clone(),
                    name,
                });
            }
        }
        if self.options.ignore_requires_closed && running.is_empty() {
            self.warn(
                None,
                "override: requires-closed check disabled (no affected app is running now)".into(),
            );
        }
        running
    }

    /// Display names of the apps `ids` that the receiver knows (id → name).
    pub(super) fn app_names<'i>(
        &self,
        ids: impl IntoIterator<Item = &'i String>,
    ) -> BTreeMap<String, String> {
        ids.into_iter()
            .filter_map(|id| self.recipe(id).map(|r| (id.clone(), r.name.clone())))
            .collect()
    }

    /// The display name of app `id`: its recipe's name, else the id.
    fn app_name(&self, id: &str) -> String {
        self.recipe(id)
            .map_or_else(|| id.to_owned(), |r| r.name.clone())
    }

    /// Recipes of selected requires-closed items with a running process.
    fn detect_running(&mut self) -> Vec<String> {
        let ids = self.selected_recipe_ids(true);
        if ids.is_empty() {
            return Vec::new();
        }
        let procs = match self.platform.running_processes() {
            Ok(procs) => procs,
            Err(e) => {
                self.warn(
                    None,
                    format!("could not list running processes ({e}); close affected apps before applying"),
                );
                return Vec::new();
            }
        };
        let os = self.platform.os();
        ids.into_iter()
            .filter(|id| {
                self.recipe(id).is_some_and(|r| {
                    r.process_names(os)
                        .any(|n| procs.iter().any(|p| p.matches_binary(n)))
                })
            })
            .map(str::to_owned)
            .collect()
    }

    /// Free space of the home filesystem, if it can be determined.
    pub(super) fn free_space(&mut self) -> Option<u64> {
        let home = self.roots.home()?.to_path_buf();
        match self.platform.free_space(&home) {
            Ok(n) => Some(n),
            Err(e) => {
                self.warn(None, format!("could not determine free space: {e}"));
                None
            }
        }
    }

    /// Every filesystem short of space is a blocker, unless the user
    /// overrides the check (then one warning per short filesystem).
    pub(super) fn check_space(&mut self, needs: &[SpaceNeed]) {
        let ignore = self.options.ignore_free_space;
        let mut ignored = false;
        for short in needs.iter().filter_map(SpaceNeed::shortfall) {
            if ignore {
                ignored = true;
                self.warn(
                    None,
                    format!("override: free space check ignored ({short})"),
                );
            } else {
                self.blockers.push(Blocker::FreeSpace(short));
            }
        }
        if ignore && !ignored {
            self.warn(None, "override: free space check disabled".into());
        }
    }
}
