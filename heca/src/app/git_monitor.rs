//! Per-frame git metadata monitor for pane runtime state.
//!
//! This complements `process_monitor`: once Phase 2/3 populate canonical
//! `Pane.runtime.cwd`, this module resolves repo metadata for that cwd,
//! caches it by repo root, and projects it back onto canonical
//! `Pane.runtime.git` before the chrome store mirrors runtime state.

use crate::app_state::AppState;
use heca_core::layout::{PaneId, Session};
use heca_core::runtime::GitInfo;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const GIT_REFRESH_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq)]
struct GitSnapshot {
    repo_root: PathBuf,
    info: GitInfo,
}

trait GitProvider {
    fn inspect(&self, cwd: &Path) -> Option<GitSnapshot>;
}

#[derive(Default)]
struct Git2Provider;

impl GitProvider for Git2Provider {
    fn inspect(&self, cwd: &Path) -> Option<GitSnapshot> {
        use git2::{BranchType, Repository, Status, StatusOptions};

        let repo = Repository::discover(cwd).ok()?;
        let repo_root = repo
            .workdir()
            .map(Path::to_path_buf)
            .or_else(|| repo.path().parent().map(Path::to_path_buf))
            .unwrap_or_else(|| repo.path().to_path_buf());

        let head = repo.head().ok();
        let branch = head
            .as_ref()
            .and_then(|head| head.shorthand())
            .map(str::to_string);
        let (ahead, behind) = head
            .as_ref()
            .and_then(|head| head.shorthand().map(str::to_string))
            .and_then(|name| repo.find_branch(&name, BranchType::Local).ok())
            .and_then(|branch| branch.upstream().ok().map(|upstream| (branch, upstream)))
            .and_then(|(branch, upstream)| {
                let local = branch.get().target()?;
                let upstream = upstream.get().target()?;
                repo.graph_ahead_behind(local, upstream).ok()
            })
            .unwrap_or((0, 0));

        let mut opts = StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_ignored(false)
            .renames_head_to_index(true);
        let statuses = repo.statuses(Some(&mut opts)).ok()?;
        let mut info = GitInfo {
            branch,
            ahead: ahead as u32,
            behind: behind as u32,
            ..GitInfo::default()
        };
        for entry in statuses.iter() {
            let status = entry.status();
            if status.is_empty() {
                continue;
            }
            if status.intersects(Status::INDEX_NEW | Status::WT_NEW) {
                info.added += 1;
            }
            if status.intersects(Status::INDEX_DELETED | Status::WT_DELETED) {
                info.deleted += 1;
            }
            if status.intersects(
                Status::INDEX_MODIFIED
                    | Status::WT_MODIFIED
                    | Status::INDEX_RENAMED
                    | Status::WT_RENAMED
                    | Status::INDEX_TYPECHANGE
                    | Status::WT_TYPECHANGE
                    | Status::CONFLICTED,
            ) {
                info.modified += 1;
            }
        }
        info.dirty = info.added > 0 || info.modified > 0 || info.deleted > 0;
        Some(GitSnapshot { repo_root, info })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PaneGitState {
    raw_cwd: Option<PathBuf>,
    canonical_cwd: Option<PathBuf>,
    repo_root: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct RepoGitCacheEntry {
    info: GitInfo,
    last_checked: Instant,
}

#[derive(Debug, Default)]
pub(crate) struct GitRuntimeCache {
    pane_state: HashMap<PaneId, PaneGitState>,
    repos: HashMap<PathBuf, RepoGitCacheEntry>,
}

struct GitSyncContext<'a, P> {
    cache: &'a mut GitRuntimeCache,
    provider: &'a P,
    now: Instant,
    refresh_after: Duration,
    live_roots: &'a mut HashSet<PathBuf>,
}

pub(crate) fn sync_pane_git_from_cwds(state: &mut AppState) {
    let provider = Git2Provider;
    sync_pane_git_from_cwds_impl(
        &mut state.session,
        &mut state.git_runtime_cache,
        &provider,
        Instant::now(),
        GIT_REFRESH_INTERVAL,
    );
}

fn sync_pane_git_from_cwds_impl<P: GitProvider>(
    session: &mut Session,
    cache: &mut GitRuntimeCache,
    provider: &P,
    now: Instant,
    refresh_after: Duration,
) {
    let mut live_panes = HashSet::new();
    let mut live_roots = HashSet::new();
    let mut ctx = GitSyncContext {
        cache,
        provider,
        now,
        refresh_after,
        live_roots: &mut live_roots,
    };

    for ws in &mut session.workspaces {
        for col in &mut ws.scrolling.columns {
            for pane in &mut col.panes {
                live_panes.insert(pane.id);
                sync_one_pane_git(&mut ctx, pane.id, &pane.runtime.cwd, &mut pane.runtime.git);
            }
        }
        for float in &mut ws.floating_panes {
            live_panes.insert(float.pane.id);
            sync_one_pane_git(
                &mut ctx,
                float.pane.id,
                &float.pane.runtime.cwd,
                &mut float.pane.runtime.git,
            );
        }
    }

    ctx.cache
        .pane_state
        .retain(|pane, _| live_panes.contains(pane));
    ctx.cache
        .repos
        .retain(|root, _| ctx.live_roots.contains(root));
}

fn sync_one_pane_git<P: GitProvider>(
    ctx: &mut GitSyncContext<'_, P>,
    pane_id: PaneId,
    cwd: &Option<PathBuf>,
    git: &mut Option<GitInfo>,
) {
    let raw_cwd = cwd.clone();
    let prev = ctx.cache.pane_state.get(&pane_id).cloned();

    let Some(raw_cwd) = raw_cwd else {
        *git = None;
        ctx.cache.pane_state.insert(
            pane_id,
            PaneGitState {
                raw_cwd: None,
                canonical_cwd: None,
                repo_root: None,
            },
        );
        return;
    };

    let cwd_changed = prev.as_ref().map(|state| state.raw_cwd.as_ref()) != Some(Some(&raw_cwd));
    let repo_root = prev.as_ref().and_then(|state| state.repo_root.clone());

    if cwd_changed {
        let canonical_cwd = fs::canonicalize(&raw_cwd).unwrap_or_else(|_| raw_cwd.clone());
        match ctx.provider.inspect(&canonical_cwd) {
            Some(snapshot) => {
                ctx.live_roots.insert(snapshot.repo_root.clone());
                ctx.cache.repos.insert(
                    snapshot.repo_root.clone(),
                    RepoGitCacheEntry {
                        info: snapshot.info.clone(),
                        last_checked: ctx.now,
                    },
                );
                *git = Some(snapshot.info.clone());
                ctx.cache.pane_state.insert(
                    pane_id,
                    PaneGitState {
                        raw_cwd: Some(raw_cwd),
                        canonical_cwd: Some(canonical_cwd),
                        repo_root: Some(snapshot.repo_root),
                    },
                );
            }
            None => {
                *git = None;
                ctx.cache.pane_state.insert(
                    pane_id,
                    PaneGitState {
                        raw_cwd: Some(raw_cwd),
                        canonical_cwd: Some(canonical_cwd),
                        repo_root: None,
                    },
                );
            }
        }
        return;
    }

    let Some(root) = repo_root else {
        *git = None;
        return;
    };
    ctx.live_roots.insert(root.clone());
    let cwd_path = prev
        .as_ref()
        .and_then(|state| state.canonical_cwd.clone())
        .unwrap_or_else(|| fs::canonicalize(&raw_cwd).unwrap_or_else(|_| raw_cwd.clone()));

    let should_refresh = ctx
        .cache
        .repos
        .get(&root)
        .map(|entry| ctx.now.duration_since(entry.last_checked) >= ctx.refresh_after)
        .unwrap_or(true);

    if should_refresh {
        match ctx.provider.inspect(&cwd_path) {
            Some(snapshot) => {
                ctx.live_roots.insert(snapshot.repo_root.clone());
                ctx.cache.repos.insert(
                    snapshot.repo_root.clone(),
                    RepoGitCacheEntry {
                        info: snapshot.info.clone(),
                        last_checked: ctx.now,
                    },
                );
                *git = Some(snapshot.info.clone());
                ctx.cache.pane_state.insert(
                    pane_id,
                    PaneGitState {
                        raw_cwd: Some(raw_cwd),
                        canonical_cwd: Some(cwd_path),
                        repo_root: Some(snapshot.repo_root),
                    },
                );
            }
            None => {
                *git = None;
                ctx.cache.pane_state.insert(
                    pane_id,
                    PaneGitState {
                        raw_cwd: Some(raw_cwd),
                        canonical_cwd: Some(cwd_path),
                        repo_root: None,
                    },
                );
            }
        }
        return;
    }

    // The `pane.git.changed` event is emitted by the existing chokepoint one
    // step later: `sync_pane_runtime_state` calls `set_pane_runtime`, whose git
    // change guard emits `ChromeEvent::PaneGitChanged` on real change only.
    *git = ctx.cache.repos.get(&root).map(|entry| entry.info.clone());
}

#[cfg(test)]
mod tests {
    use super::{GitInfo, GitProvider, GitRuntimeCache, GitSnapshot, sync_pane_git_from_cwds_impl};
    use heca_core::layout::{
        ColumnWidth, LayoutOptions, Pane, PaneId, Point, Session, SessionId, Size,
        workspace::FloatingPane,
    };
    use std::cell::Cell;
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    struct FakeGitProvider {
        calls: Cell<u32>,
        snapshots: HashMap<PathBuf, Option<GitSnapshot>>,
    }

    impl FakeGitProvider {
        fn new(snapshots: HashMap<PathBuf, Option<GitSnapshot>>) -> Self {
            Self {
                calls: Cell::new(0),
                snapshots,
            }
        }

        fn calls(&self) -> u32 {
            self.calls.get()
        }
    }

    impl GitProvider for FakeGitProvider {
        fn inspect(&self, cwd: &Path) -> Option<GitSnapshot> {
            self.calls.set(self.calls.get() + 1);
            self.snapshots.get(cwd).cloned().flatten()
        }
    }

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(label: &str) -> Self {
            let mut path = std::env::temp_dir();
            path.push(format!(
                "heca-git-monitor-{}-{}",
                label,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("clock ok")
                    .as_nanos()
            ));
            fs::create_dir_all(&path).expect("temp dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn session_with_tiled_pane(id: PaneId) -> Session {
        let mut session = Session::new(
            SessionId(1),
            Size::new(1280.0, 800.0),
            1.0,
            LayoutOptions::default(),
        );
        let ws = session
            .active_workspace_mut()
            .expect("session should create an initial workspace");
        ws.add_pane(
            Pane::new(id, "editor"),
            None,
            true,
            ColumnWidth::Proportion(0.5),
            heca_core::layout::ColumnId(id.0),
        );
        session
    }

    #[test]
    fn monitor_projects_provider_git_info_into_canonical_runtime() {
        let pane_id = PaneId(10);
        let repo = PathBuf::from("/repo/project");
        let mut session = session_with_tiled_pane(pane_id);
        session
            .active_workspace_mut()
            .expect("workspace")
            .find_pane_mut(pane_id)
            .expect("pane")
            .runtime
            .cwd = Some(repo.clone());

        let info = GitInfo {
            branch: Some("main".into()),
            ahead: 1,
            behind: 2,
            added: 3,
            modified: 4,
            deleted: 5,
            dirty: true,
        };
        let provider = FakeGitProvider::new(HashMap::from([(
            repo.clone(),
            Some(GitSnapshot {
                repo_root: PathBuf::from("/repo"),
                info: info.clone(),
            }),
        )]));
        let mut cache = GitRuntimeCache::default();

        sync_pane_git_from_cwds_impl(
            &mut session,
            &mut cache,
            &provider,
            Instant::now(),
            Duration::from_secs(2),
        );

        let runtime = session
            .active_workspace()
            .and_then(|ws| ws.find_pane(pane_id))
            .expect("pane")
            .runtime
            .clone();
        assert_eq!(runtime.git, Some(info));
        assert_eq!(provider.calls(), 1);
    }

    #[test]
    fn monitor_shares_repo_cache_across_panes() {
        let repo_a = PathBuf::from("/repo/a");
        let repo_b = PathBuf::from("/repo/b");
        let root = PathBuf::from("/repo");
        let mut session = session_with_tiled_pane(PaneId(10));
        {
            let ws = session.active_workspace_mut().expect("workspace");
            ws.find_pane_mut(PaneId(10)).expect("pane").runtime.cwd = Some(repo_a.clone());
            ws.floating_panes.push(FloatingPane {
                pane: {
                    let mut pane = Pane::new(PaneId(20), "float");
                    pane.runtime.cwd = Some(repo_b.clone());
                    pane
                },
                position: Point::new(0.0, 0.0),
                size: Size::new(100.0, 100.0),
                is_active: false,
                original_column_idx: None,
                original_pane_idx: None,
            });
        }

        let info = GitInfo {
            branch: Some("main".into()),
            dirty: true,
            ..GitInfo::default()
        };
        let provider = FakeGitProvider::new(HashMap::from([
            (
                repo_a.clone(),
                Some(GitSnapshot {
                    repo_root: root.clone(),
                    info: info.clone(),
                }),
            ),
            (
                repo_b.clone(),
                Some(GitSnapshot {
                    repo_root: root.clone(),
                    info: info.clone(),
                }),
            ),
        ]));
        let mut cache = GitRuntimeCache::default();
        let now = Instant::now();

        sync_pane_git_from_cwds_impl(
            &mut session,
            &mut cache,
            &provider,
            now,
            Duration::from_secs(10),
        );
        assert_eq!(provider.calls(), 2);

        sync_pane_git_from_cwds_impl(
            &mut session,
            &mut cache,
            &provider,
            now + Duration::from_secs(1),
            Duration::from_secs(10),
        );
        assert_eq!(provider.calls(), 2);
    }

    #[test]
    fn monitor_refreshes_repo_after_debounce_window() {
        let repo = PathBuf::from("/repo/project");
        let root = PathBuf::from("/repo");
        let mut session = session_with_tiled_pane(PaneId(10));
        session
            .active_workspace_mut()
            .expect("workspace")
            .find_pane_mut(PaneId(10))
            .expect("pane")
            .runtime
            .cwd = Some(repo.clone());

        let provider = FakeGitProvider::new(HashMap::from([(
            repo.clone(),
            Some(GitSnapshot {
                repo_root: root,
                info: GitInfo {
                    branch: Some("main".into()),
                    ..GitInfo::default()
                },
            }),
        )]));
        let mut cache = GitRuntimeCache::default();
        let now = Instant::now();

        sync_pane_git_from_cwds_impl(
            &mut session,
            &mut cache,
            &provider,
            now,
            Duration::from_secs(2),
        );
        sync_pane_git_from_cwds_impl(
            &mut session,
            &mut cache,
            &provider,
            now + Duration::from_secs(3),
            Duration::from_secs(2),
        );
        assert_eq!(provider.calls(), 2);
    }

    #[test]
    fn git2_provider_reports_branch_and_dirty_counts_from_temp_repo() {
        let temp = TempDir::new("dirty-repo");
        let repo = git2::Repository::init(temp.path()).expect("init repo");
        let file = temp.path().join("note.txt");
        fs::write(&file, "hello\n").expect("write file");
        {
            let mut index = repo.index().expect("index");
            index.add_path(Path::new("note.txt")).expect("add");
            index.write().expect("write index");
            let tree_id = index.write_tree().expect("tree");
            let tree = repo.find_tree(tree_id).expect("tree");
            let sig = git2::Signature::now("heca", "heca@example.com").expect("sig");
            repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
                .expect("commit");
        }
        fs::write(&file, "hello\nworld\n").expect("modify file");
        fs::write(temp.path().join("new.txt"), "new\n").expect("new file");
        repo.status_file(Path::new("note.txt"))
            .expect("status file");

        let snapshot = super::Git2Provider
            .inspect(temp.path())
            .expect("git snapshot");

        assert_eq!(
            fs::canonicalize(&snapshot.repo_root).expect("canonical repo root"),
            fs::canonicalize(temp.path()).expect("canonical temp path"),
        );
        assert!(
            matches!(
                snapshot.info.branch.as_deref(),
                Some("main") | Some("master")
            ),
            "unexpected branch {:?}",
            snapshot.info.branch,
        );
        assert!(snapshot.info.modified >= 1);
        assert!(snapshot.info.added >= 1);
        assert!(snapshot.info.dirty);
    }
}
