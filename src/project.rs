use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::config::Config;
use crate::conversation::Conversation;
use crate::embedding::model::{self, EmbeddingModel};
use crate::error::{ConversationError, ForgetError, GetError, InitError, RebuildError};
use crate::index::cache::IndexCache;
use crate::index::segment_index::SegmentIndex;
use crate::segment::lifecycle;
use crate::storage::db::Database;
use crate::storage::queries;
use crate::types::{
    ConversationInfo, ForgetOutcome, ForgetPlan, ForgetTarget, NodeContent, ProjectStats, Scope,
    SegmentImpact,
};

pub struct Moerae {
    pub(crate) db: Database,
    pub(crate) model: EmbeddingModel,
    pub(crate) config: Config,
    pub(crate) cache: RefCell<IndexCache>,
    pub(crate) active_conversations: Rc<RefCell<HashSet<String>>>,
}

impl Moerae {
    pub fn db(&self) -> &Database {
        &self.db
    }

    pub fn model(&self) -> &EmbeddingModel {
        &self.model
    }

    pub fn init(project_id: &str) -> Result<Self, InitError> {
        let model_path = model::default_model_path();
        Self::init_internal(project_id, None, Some(&model_path))
    }

    pub fn builder(project_id: &str) -> MoeraeBuilder {
        MoeraeBuilder {
            project_id: project_id.to_string(),
            config: None,
            model_path: None,
        }
    }

    fn init_internal(
        project_id: &str,
        config_override: Option<Config>,
        model_path: Option<&Path>,
    ) -> Result<Self, InitError> {
        let db = Database::open(project_id)?;

        // Load or override config
        let config = match config_override {
            Some(c) => {
                c.validate().map_err(InitError::InvalidConfig)?;
                db.store_config(&c)?;
                c
            }
            None => {
                let c = db.load_config()?;
                c.validate().map_err(InitError::InvalidConfig)?;
                c
            }
        };

        // Store project name in config for discovery
        let _ = db.conn.execute(
            "INSERT INTO config (id, data) VALUES (1, json_object('_project_id', ?1))
             ON CONFLICT(id) DO UPDATE SET data = json_set(data, '$._project_id', ?1)",
            [project_id],
        );

        // Load embedding model
        let default_path = model::default_model_path();
        let path = model_path.unwrap_or(&default_path);
        let model = EmbeddingModel::load(path)?;

        let cache = RefCell::new(IndexCache::new(config.max_cached_indexes));

        // Orphan file cleanup
        cleanup_orphan_files(&db)?;

        // Finish any rewrite interrupted between save_to(tmp) and the rename
        lifecycle::recover_interrupted_rewrites(&db)
            .map_err(|e| InitError::StorageInit(std::io::Error::other(e)))?;

        // Init-time eviction check
        run_init_eviction(&db, &config, &mut cache.borrow_mut())?;

        Ok(Self {
            db,
            model,
            config,
            cache,
            active_conversations: Rc::new(RefCell::new(HashSet::new())),
        })
    }

    pub fn create_conversation(&self) -> Result<Conversation<'_>, ConversationError> {
        let uuid = uuid::Uuid::new_v4().to_string();
        queries::insert_conversation(&self.db.conn, &uuid)
            .map_err(|e| ConversationError::StorageWrite(std::io::Error::other(e)))?;

        self.active_conversations.borrow_mut().insert(uuid.clone());

        Conversation::new(self, uuid)
    }

    pub fn conversation(&self, uuid: &str) -> Result<Conversation<'_>, ConversationError> {
        if !queries::conversation_exists(&self.db.conn, uuid)
            .map_err(|e| ConversationError::StorageRead(std::io::Error::other(e)))?
        {
            return Err(ConversationError::NotFound(uuid.to_string()));
        }

        // Close orphan open segments from prior sessions
        lifecycle::close_orphan_segments(
            &self.db,
            uuid,
            &mut self.cache.borrow_mut(),
            &self.config,
        )
        .map_err(|e| ConversationError::StorageRead(std::io::Error::other(e)))?;

        self.active_conversations.borrow_mut().insert(uuid.to_string());

        Conversation::load(self, uuid.to_string())
    }

    pub fn delete_conversation(&self, uuid: &str) -> Result<(), ConversationError> {
        if self.active_conversations.borrow().contains(uuid) {
            return Err(ConversationError::InUse(uuid.to_string()));
        }

        // Get segment IDs before deletion (for file cleanup)
        let segment_ids = queries::get_conversation_segments(&self.db.conn, uuid)
            .map_err(|e| ConversationError::StorageRead(std::io::Error::other(e)))?;

        // Invalidate cache + delete files
        let mut cache = self.cache.borrow_mut();
        for seg_id in &segment_ids {
            cache.invalidate(*seg_id);
            let _ = SegmentIndex::remove_file(&self.db.index_path(*seg_id));
        }

        // Delete conversation (cascades to segments and nodes)
        queries::delete_conversation(&self.db.conn, uuid)
            .map_err(|e| ConversationError::StorageWrite(std::io::Error::other(e)))?;

        Ok(())
    }

    pub fn list_conversations(&self) -> Result<Vec<ConversationInfo>, ConversationError> {
        queries::list_conversations(&self.db.conn)
            .map_err(|e| ConversationError::StorageRead(std::io::Error::other(e)))
    }

    pub fn segment_stats(&self) -> Result<ProjectStats, ConversationError> {
        queries::get_project_stats(&self.db.conn, self.model.model_id())
            .map_err(|e| ConversationError::StorageRead(std::io::Error::other(e)))
    }

    /// Plans the removal of specific nodes, named by id. Does not touch the model.
    pub fn plan_forget_nodes(&self, node_ids: &[i64]) -> Result<ForgetPlan, ForgetError> {
        let locations = queries::get_nodes_info(&self.db.conn, node_ids)
            .map_err(|e| ForgetError::StorageRead(std::io::Error::other(e)))?;

        // An id the caller named but that does not exist is a mistake worth reporting,
        // unlike a stale plan applied later (where a vanished node is simply skipped).
        if let Some(missing) = node_ids.iter().find(|id| {
            !locations.iter().any(|l| l.node_id == **id)
        }) {
            return Err(ForgetError::NodeNotFound(*missing));
        }

        let targets = locations
            .into_iter()
            .map(|l| ForgetTarget {
                node_id: l.node_id,
                segment_id: l.segment_id,
                conversation_id: l.conversation_id,
                data: l.data,
                metadata: l.metadata,
                score: None,
            })
            .collect();

        self.build_plan(targets, false, 0)
    }

    /// Plans the removal of nodes semantically matching `query` at or above `min_score`.
    ///
    /// Takes a conversation id rather than a handle on purpose: opening one would insert
    /// into `active_conversations`, and dropping it would clear the flag for a caller's
    /// live handle. Planning must leave that set alone.
    pub fn plan_forget_matching(
        &self,
        conversation_id: Option<&str>,
        query: &str,
        scope: Option<Scope>,
        limit: Option<usize>,
        min_score: f32,
    ) -> Result<ForgetPlan, ForgetError> {
        let conv = match conversation_id {
            Some(uuid) => {
                if !queries::conversation_exists(&self.db.conn, uuid)
                    .map_err(|e| ForgetError::StorageRead(std::io::Error::other(e)))?
                {
                    return Err(ForgetError::ConversationNotFound(uuid.to_string()));
                }
                // Repair segments left open by a previous process before searching.
                // `Moerae::conversation` does this, but we cannot call it: it inserts
                // into active_conversations, and the matching drop would clear the flag
                // for a caller's live handle. Without this, a segment written by an
                // earlier `moerae put` has no index file yet and is silently skipped —
                // so a forget planned right after a put would match nothing.
                lifecycle::close_orphan_segments(
                    &self.db,
                    uuid,
                    &mut self.cache.borrow_mut(),
                    &self.config,
                )
                .map_err(|e| ForgetError::StorageRead(std::io::Error::other(e)))?;

                Conversation::load(self, uuid.to_string())
                    .map_err(|e| ForgetError::StorageRead(std::io::Error::other(e.to_string())))?
            }
            // Project scope reads promoted segments across all conversations, so no real
            // conversation is needed. Conversation::new inserts no row, so this leaves
            // nothing behind — unlike `search`, which creates one.
            None => Conversation::new(self, uuid::Uuid::new_v4().to_string())
                .map_err(|e| ForgetError::StorageRead(std::io::Error::other(e.to_string())))?,
        };

        let results = conv.search_no_boost(query, scope, limit)?;
        let skipped_mismatched = results.skipped_mismatched;
        let has_more = results.has_more;

        let targets: Vec<ForgetTarget> = results
            .items
            .into_iter()
            .filter(|r| r.score >= min_score)
            .map(|r| ForgetTarget {
                node_id: r.node_id,
                segment_id: r.segment_id,
                conversation_id: r.conversation_id,
                data: r.data,
                metadata: r.metadata,
                score: Some(r.score),
            })
            .collect();

        if targets.is_empty() {
            return Err(ForgetError::NoMatch);
        }

        self.build_plan(targets, has_more, skipped_mismatched)
    }

    /// Groups targets by segment and records what each segment would lose.
    fn build_plan(
        &self,
        targets: Vec<ForgetTarget>,
        has_more: bool,
        skipped_mismatched: usize,
    ) -> Result<ForgetPlan, ForgetError> {
        let mut segment_ids: Vec<i64> = targets.iter().map(|t| t.segment_id).collect();
        segment_ids.sort_unstable();
        segment_ids.dedup();

        let mut impacts = Vec::with_capacity(segment_ids.len());
        let mut emptied_segments = Vec::new();

        for segment_id in segment_ids {
            let info = queries::get_segment_info(&self.db.conn, segment_id)
                .map_err(|e| ForgetError::StorageRead(std::io::Error::other(e)))?;

            let nodes_removed = targets
                .iter()
                .filter(|t| t.segment_id == segment_id)
                .count() as i64;

            let actual = queries::count_segment_nodes(&self.db.conn, segment_id)
                .map_err(|e| ForgetError::StorageRead(std::io::Error::other(e)))?;

            if nodes_removed >= actual {
                emptied_segments.push(segment_id);
            }

            impacts.push(SegmentImpact {
                segment_id,
                node_count_before: actual,
                nodes_removed,
                state: info.state,
                persist: info.persist,
                promoted: info.promoted,
            });
        }

        Ok(ForgetPlan {
            targets,
            impacts,
            emptied_segments,
            has_more,
            skipped_mismatched,
        })
    }

    /// Executes a plan. Does not touch the model.
    ///
    /// Nodes that have disappeared since the plan was built are silently skipped —
    /// `node_id` is AUTOINCREMENT and never reused, so a stale plan can target a missing
    /// node but never a different one.
    pub fn apply_forget(&self, plan: &ForgetPlan) -> Result<ForgetOutcome, ForgetError> {
        // Group targets by segment.
        let mut by_segment: Vec<(i64, Vec<i64>)> = Vec::new();
        for target in &plan.targets {
            match by_segment.iter_mut().find(|(s, _)| *s == target.segment_id) {
                Some((_, ids)) => ids.push(target.node_id),
                None => by_segment.push((target.segment_id, vec![target.node_id])),
            }
        }

        // Refuse only the genuinely dangerous case: the open segment of a conversation
        // that still has a live handle. Rewriting a closed segment underneath one is safe.
        for (segment_id, _) in &by_segment {
            let info = match queries::get_segment_info(&self.db.conn, *segment_id) {
                Ok(i) => i,
                Err(_) => continue, // segment already gone; nothing to guard
            };
            if info.state == "open"
                && self
                    .active_conversations
                    .borrow()
                    .contains(&info.conversation_id)
            {
                return Err(ForgetError::SegmentInUse {
                    segment_id: *segment_id,
                    conversation_id: info.conversation_id,
                });
            }
        }

        let mut cache = self.cache.borrow_mut();
        let mut outcome = ForgetOutcome {
            nodes_removed: 0,
            segments_rewritten: 0,
            segments_dropped: 0,
        };

        for (segment_id, node_ids) in &by_segment {
            if queries::get_segment_info(&self.db.conn, *segment_id).is_err() {
                continue; // segment vanished since planning
            }

            let result = lifecycle::forget_nodes_in_segment(
                &self.db,
                *segment_id,
                node_ids,
                &self.config,
                &mut cache,
            )
            .map_err(ForgetError::RebuildFailed)?;

            match result {
                lifecycle::ForgetSegmentOutcome::Rewritten { removed, .. } => {
                    outcome.nodes_removed += removed;
                    outcome.segments_rewritten += 1;
                }
                lifecycle::ForgetSegmentOutcome::Dropped { removed } => {
                    outcome.nodes_removed += removed;
                    outcome.segments_dropped += 1;
                }
            }
        }

        Ok(outcome)
    }

    /// Convenience: plan and apply in one step, for explicit node ids.
    pub fn forget_nodes(&self, node_ids: &[i64]) -> Result<ForgetOutcome, ForgetError> {
        let plan = self.plan_forget_nodes(node_ids)?;
        self.apply_forget(&plan)
    }

    pub fn rebuild_mismatched_segments(&self) -> Result<(), RebuildError> {
        let mismatched = queries::get_mismatched_segments(&self.db.conn, self.model.model_id())
            .map_err(|e| RebuildError::StorageWrite(std::io::Error::other(e)))?;

        if mismatched.is_empty() {
            return Err(RebuildError::NoMismatchedSegments);
        }

        for segment_id in &mismatched {
            // Get all nodes for this segment
            let _nodes = queries::get_segment_embeddings(&self.db.conn, *segment_id)
                .map_err(|e| RebuildError::StorageWrite(std::io::Error::other(e)))?;

            // Re-embed each node's content
            // (For rebuild, we read the original data from nodes table and re-embed)
            let node_data = self.get_segment_node_data(*segment_id)?;

            let tmp_path = self.db.index_tmp_path(*segment_id);
            let final_path = self.db.index_path(*segment_id);

            let index = SegmentIndex::create(
                &tmp_path,
                node_data.len(),
                self.model.dimensions(),
            )
            .map_err(|e| RebuildError::EmbeddingFailed(e))?;

            for (node_id, data, metadata) in &node_data {
                // Determine which text to embed (data or metadata based on token count)
                let text_to_embed = if let Some(meta) = metadata {
                    // If metadata exists and original data was too long, embed metadata
                    let token_count = self.model.token_count(data)
                        .map_err(|e| RebuildError::EmbeddingFailed(e))?;
                    if token_count > self.config.max_indexable_tokens {
                        meta.as_str()
                    } else {
                        data.as_str()
                    }
                } else {
                    data.as_str()
                };

                let embedding = self.model.embed(text_to_embed)
                    .map_err(|e| RebuildError::EmbeddingFailed(e))?;

                index
                    .add(*node_id as u64, &embedding)
                    .map_err(|e| RebuildError::EmbeddingFailed(e))?;

                // Update embedding blob in SQLite
                let emb_bytes = EmbeddingModel::embedding_to_bytes(&embedding);
                self.db.conn.execute(
                    "UPDATE nodes SET embedding = ?1 WHERE node_id = ?2",
                    rusqlite::params![emb_bytes, node_id],
                ).map_err(|e| RebuildError::StorageWrite(std::io::Error::other(e)))?;
            }

            index.save_to(&tmp_path).map_err(|e| RebuildError::EmbeddingFailed(e))?;
            drop(index);

            std::fs::rename(&tmp_path, &final_path)
                .map_err(RebuildError::StorageWrite)?;

            // Update model_id for the segment
            self.db.conn.execute(
                "UPDATE segments SET model_id = ?1 WHERE segment_id = ?2",
                rusqlite::params![self.model.model_id(), segment_id],
            ).map_err(|e| RebuildError::StorageWrite(std::io::Error::other(e)))?;

            // Invalidate cache
            self.cache.borrow_mut().invalidate(*segment_id);
        }

        Ok(())
    }

    pub fn get(&self, node_id: i64) -> Result<NodeContent, GetError> {
        queries::get_node(&self.db.conn, node_id)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => GetError::NotFound(node_id),
                _ => GetError::StorageRead(std::io::Error::other(e)),
            })
    }

    pub(crate) fn unregister_conversation(&self, uuid: &str) {
        self.active_conversations.borrow_mut().remove(uuid);
    }

    fn get_segment_node_data(&self, segment_id: i64) -> Result<Vec<(i64, String, Option<String>)>, RebuildError> {
        let mut stmt = self.db.conn.prepare(
            "SELECT node_id, data, metadata FROM nodes WHERE segment_id = ?1",
        ).map_err(|e| RebuildError::StorageWrite(std::io::Error::other(e)))?;

        let rows = stmt.query_map([segment_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        }).map_err(|e| RebuildError::StorageWrite(std::io::Error::other(e)))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| RebuildError::StorageWrite(std::io::Error::other(e)))
    }
}

pub struct MoeraeBuilder {
    project_id: String,
    config: Option<Config>,
    model_path: Option<PathBuf>,
}

impl MoeraeBuilder {
    pub fn config(mut self, config: Config) -> Self {
        self.config = Some(config);
        self
    }

    pub fn model_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.model_path = Some(path.into());
        self
    }

    pub fn build(self) -> Result<Moerae, InitError> {
        Moerae::init_internal(
            &self.project_id,
            self.config,
            self.model_path.as_deref(),
        )
    }
}

fn cleanup_orphan_files(db: &Database) -> Result<(), InitError> {
    let valid_ids: HashSet<i64> = queries::get_all_segment_ids(&db.conn)
        .map_err(|e| InitError::StorageInit(std::io::Error::other(e)))?
        .into_iter()
        .collect();

    if let Ok(entries) = std::fs::read_dir(&db.indexes_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                // Parse segment_id from filename like "123.usearch" or "123.usearch.tmp"
                let stem = name
                    .strip_suffix(".usearch.tmp")
                    .or_else(|| name.strip_suffix(".usearch"));

                if let Some(id_str) = stem {
                    if let Ok(id) = id_str.parse::<i64>() {
                        if !valid_ids.contains(&id) {
                            let _ = std::fs::remove_file(&path);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn run_init_eviction(db: &Database, config: &Config, cache: &mut IndexCache) -> Result<(), InitError> {
    let over_limit = queries::get_conversations_over_segment_limit(&db.conn, config.max_segments)
        .map_err(|e| InitError::StorageInit(std::io::Error::other(e)))?;

    for conv_id in &over_limit {
        lifecycle::run_eviction(db, conv_id, config, cache)
            .map_err(|e| InitError::StorageInit(std::io::Error::other(e)))?;
    }

    Ok(())
}
