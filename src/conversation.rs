use std::collections::HashSet;

use crate::embedding::model::EmbeddingModel;
use crate::error::{ConversationError, PutError, SearchError};
use crate::project::Moerae;
use crate::segment::manager::SegmentManager;
use crate::storage::queries;
use crate::types::{
    SearchResult, SearchResultDebug, SearchResults, SearchResultsDebug, Scope,
};

pub struct Conversation<'a> {
    moerae: &'a Moerae,
    pub uuid: String,
    manager: SegmentManager,
}

impl<'a> Conversation<'a> {
    pub(crate) fn new(moerae: &'a Moerae, uuid: String) -> Result<Self, ConversationError> {
        let manager = SegmentManager::new(uuid.clone());
        Ok(Self {
            moerae,
            uuid,
            manager,
        })
    }

    pub(crate) fn load(moerae: &'a Moerae, uuid: String) -> Result<Self, ConversationError> {
        let manager = SegmentManager::load(&moerae.db, uuid.clone())
            .map_err(|e| ConversationError::StorageRead(std::io::Error::other(e)))?;
        Ok(Self {
            moerae,
            uuid,
            manager,
        })
    }

    pub fn put(
        &mut self,
        data: &str,
        metadata: Option<&str>,
        persist: bool,
    ) -> Result<(), PutError> {
        // Validate non-empty
        if data.trim().is_empty() {
            return Err(PutError::EmptyData);
        }

        let config = &self.moerae.config;
        let model = &self.moerae.model;

        // Token count check
        let token_count = model
            .token_count(data)
            .map_err(|e| PutError::EmbeddingFailed(e))?;

        // Determine what to embed
        let (text_to_embed, store_metadata) = if token_count > config.max_indexable_tokens {
            // Large content — metadata required
            let meta = metadata.ok_or(PutError::MetadataRequired {
                token_count,
                max: config.max_indexable_tokens,
            })?;

            // Validate metadata length
            let meta_tokens = model
                .token_count(meta)
                .map_err(|e| PutError::EmbeddingFailed(e))?;
            if meta_tokens > config.max_indexable_tokens {
                return Err(PutError::MetadataTooLong {
                    token_count: meta_tokens,
                    max: config.max_indexable_tokens,
                });
            }

            (meta, metadata)
        } else {
            (data, metadata)
        };

        // Get or create open segment
        let segment_id = self.manager.get_or_create_open_segment(
            &self.moerae.db,
            persist,
            config,
            &mut self.moerae.cache.borrow_mut(),
            model.model_id(),
            model.dimensions(),
        )?;

        // Check duplicate
        let content_hash = queries::compute_content_hash(data, metadata);
        let is_dup = queries::check_duplicate(&self.moerae.db.conn, segment_id, &content_hash)
            .map_err(|e| PutError::StorageWrite(std::io::Error::other(e)))?;
        if is_dup {
            return Ok(()); // Idempotent
        }

        // Embed
        let embedding = model.embed(text_to_embed).map_err(PutError::EmbeddingFailed)?;
        let embedding_bytes = EmbeddingModel::embedding_to_bytes(&embedding);

        // Begin transaction: SQL insert + usearch insert
        let node_id = {
            let tx = self.moerae.db.conn.unchecked_transaction()
                .map_err(|e| PutError::StorageWrite(std::io::Error::other(e)))?;

            let node_id = queries::insert_node(
                &tx,
                segment_id,
                data,
                store_metadata,
                &embedding_bytes,
                &content_hash,
            )
            .map_err(|e| PutError::StorageWrite(std::io::Error::other(e)))?;

            // Insert into usearch
            let mut cache = self.moerae.cache.borrow_mut();
            let index = cache
                .get_or_load(segment_id, &self.moerae.db.index_path(segment_id))
                .map_err(|e| PutError::StorageWrite(std::io::Error::other(e)))?;

            if let Err(e) = index.add(node_id as u64, &embedding) {
                // usearch failed — rollback SQLite
                drop(cache);
                tx.rollback()
                    .map_err(|e| PutError::StorageWrite(std::io::Error::other(e)))?;
                return Err(PutError::StorageWrite(std::io::Error::other(e)));
            }

            let count = queries::increment_segment_node_count(&tx, segment_id)
                .map_err(|e| PutError::StorageWrite(std::io::Error::other(e)))?;

            drop(cache);
            tx.commit()
                .map_err(|e| PutError::StorageWrite(std::io::Error::other(e)))?;

            count
        };

        // Post-put: check capacity, eviction
        self.manager.handle_post_put(
            &self.moerae.db,
            segment_id,
            node_id,
            persist,
            config,
            &mut self.moerae.cache.borrow_mut(),
        )?;

        Ok(())
    }

    pub fn search(
        &self,
        query: &str,
        scope: Option<Scope>,
        limit: Option<usize>,
    ) -> Result<SearchResults, SearchError> {
        let (items_with_scores, has_more, skipped_mismatched) =
            self.search_internal(query, scope, limit)?;

        let items = items_with_scores
            .into_iter()
            .map(|(node_id, data, metadata, _score, _seg_id, _conv_id)| SearchResult {
                node_id,
                data,
                metadata,
            })
            .collect();

        Ok(SearchResults {
            items,
            has_more,
            skipped_mismatched,
        })
    }

    pub fn search_debug(
        &self,
        query: &str,
        scope: Option<Scope>,
        limit: Option<usize>,
    ) -> Result<SearchResultsDebug, SearchError> {
        let (items_with_scores, has_more, skipped_mismatched) =
            self.search_internal(query, scope, limit)?;

        let items = items_with_scores
            .into_iter()
            .map(
                |(node_id, data, metadata, score, segment_id, conversation_id)| {
                    SearchResultDebug {
                        node_id,
                        data,
                        metadata,
                        score,
                        segment_id,
                        conversation_id,
                    }
                },
            )
            .collect();

        Ok(SearchResultsDebug {
            items,
            has_more,
            skipped_mismatched,
        })
    }

    #[allow(clippy::type_complexity)]
    fn search_internal(
        &self,
        query: &str,
        scope: Option<Scope>,
        limit: Option<usize>,
    ) -> Result<(Vec<(i64, String, Option<String>, f32, i64, String)>, bool, usize), SearchError> {
        // Validate
        if query.trim().is_empty() {
            return Err(SearchError::EmptyQuery);
        }

        let model = &self.moerae.model;
        let config = &self.moerae.config;

        // Check query length
        let token_count = model.token_count(query).map_err(SearchError::EmbeddingFailed)?;
        if token_count > model.max_tokens() {
            return Err(SearchError::QueryTooLong {
                token_count,
                max: model.max_tokens(),
            });
        }

        let limit = limit.unwrap_or(config.default_search_limit);

        // Embed query
        let query_embedding = model.embed(query).map_err(SearchError::EmbeddingFailed)?;

        // Determine which segments to search
        let scope = scope.unwrap_or(Scope::Conversation);
        let (_segment_ids, segment_conv_map) = match scope {
            Scope::Conversation => {
                let ids = self.manager.conversation_segments().to_vec();
                let map: Vec<(i64, String)> = ids.iter().map(|id| (*id, self.uuid.clone())).collect();
                (ids, map)
            }
            Scope::Project => {
                // Query promoted segments directly from SQLite
                let ids = queries::get_promoted_segments(&self.moerae.db.conn)
                    .map_err(|e| SearchError::IndexCorrupted(e.to_string()))?;
                // Get conversation_id for each
                let map: Vec<(i64, String)> = ids
                    .iter()
                    .filter_map(|id| {
                        queries::get_segment_info(&self.moerae.db.conn, *id)
                            .ok()
                            .map(|info| (*id, info.conversation_id))
                    })
                    .collect();
                let ids: Vec<i64> = map.iter().map(|(id, _)| *id).collect();
                (ids, map)
            }
        };

        // Search each segment, skip mismatched
        let mut all_candidates: Vec<(u64, f32, i64, String)> = Vec::new(); // (node_id, distance, segment_id, conv_id)
        let mut skipped_mismatched = 0;
        let mut cache = self.moerae.cache.borrow_mut();

        let current_model_id = model.model_id();

        for (seg_id, conv_id) in &segment_conv_map {
            // Check model mismatch
            if let Ok(info) = queries::get_segment_info(&self.moerae.db.conn, *seg_id) {
                if info.model_id != current_model_id {
                    skipped_mismatched += 1;
                    continue;
                }
            }

            let index_path = self.moerae.db.index_path(*seg_id);

            // Check cache first (open segments may not have files on disk yet)
            let index_result = if cache.contains(*seg_id) {
                cache.get_or_load(*seg_id, &index_path)
            } else if index_path.exists() {
                cache.get_or_load(*seg_id, &index_path)
            } else {
                continue;
            };

            match index_result {
                Ok(index) => {
                    let results = index.search(&query_embedding, limit + 1);
                    for (key, distance) in results {
                        all_candidates.push((key, distance, *seg_id, conv_id.clone()));
                    }
                }
                Err(_) => continue, // Skip corrupted indexes
            }
        }

        drop(cache);

        // Sort by distance ascending (lower = better)
        all_candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let has_more = all_candidates.len() > limit;
        all_candidates.truncate(limit);

        // Convert to results with data from SQLite
        let mut results = Vec::with_capacity(all_candidates.len());
        // Track distinct segments hit for relevancy update
        let mut hit_segments: HashSet<i64> = HashSet::new();

        for (node_id, distance, seg_id, conv_id) in &all_candidates {
            let node_id = *node_id as i64;
            hit_segments.insert(*seg_id);

            // Fetch node data
            match queries::get_node(&self.moerae.db.conn, node_id) {
                Ok(content) => {
                    // For large nodes, SearchResult.data = metadata (what was embedded)
                    // For small nodes, SearchResult.data = original data
                    let (data, metadata) = if let Some(ref meta) = content.metadata {
                        let token_count = model.token_count(&content.data)
                            .unwrap_or(config.max_indexable_tokens + 1);
                        if token_count > config.max_indexable_tokens {
                            // Large node: return metadata as data
                            (meta.clone(), None)
                        } else {
                            (content.data, content.metadata)
                        }
                    } else {
                        (content.data, content.metadata)
                    };

                    // Convert cosine distance to similarity
                    let similarity = 1.0 - (distance / 2.0);

                    results.push((node_id, data, metadata, similarity, *seg_id, conv_id.clone()));
                }
                Err(_) => continue, // Node was evicted between search and fetch
            }
        }

        // Update relevancy: +1 hit_boost per distinct segment, in one transaction
        if !hit_segments.is_empty() {
            let _ = self.moerae.db.conn.unchecked_transaction().and_then(|tx| {
                for seg_id in &hit_segments {
                    let _ = queries::update_segment_hit(&tx, *seg_id, config.hit_boost);
                }
                tx.commit()
            });
        }

        Ok((results, has_more, skipped_mismatched))
    }
}

impl Drop for Conversation<'_> {
    fn drop(&mut self) {
        self.moerae.unregister_conversation(&self.uuid);
    }
}
