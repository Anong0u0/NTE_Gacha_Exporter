impl ProtocolAssembler {
    pub(crate) fn add_blocks(&mut self, blocks: impl IntoIterator<Item = ParsedBlock>) {
        let _ = self.apply_blocks(blocks);
    }

    #[cfg(any(windows, test))]
    pub(crate) fn add_blocks_with_update(
        &mut self,
        blocks: impl IntoIterator<Item = ParsedBlock>,
    ) -> AssemblerUpdate {
        let (rows, new_rows) = self.apply_blocks_with_optional_rows(blocks);
        AssemblerUpdate { rows, new_rows }
    }

    fn apply_blocks(
        &mut self,
        blocks: impl IntoIterator<Item = ParsedBlock>,
    ) -> (Vec<ParsedRow>, Vec<ParsedRow>) {
        let (rows, new_rows) = self.apply_blocks_with_optional_rows(blocks);
        (rows.unwrap_or_else(|| self.rows_cache.clone()), new_rows)
    }

    fn apply_blocks_with_optional_rows(
        &mut self,
        blocks: impl IntoIterator<Item = ParsedBlock>,
    ) -> (Option<Vec<ParsedRow>>, Vec<ParsedRow>) {
        self.refresh_rows();
        let previous_rows = std::mem::take(&mut self.rows_cache);
        let mut changed = false;
        for block in blocks {
            changed |= self.add_block(block);
        }
        if !changed {
            self.rows_cache = previous_rows;
            return (None, Vec::new());
        }
        self.refresh_rows();
        let rows = self.rows_cache.clone();
        let new_rows = new_prefix_rows(&previous_rows, &rows);
        (Some(rows), new_rows)
    }

    pub fn add_block(&mut self, block: ParsedBlock) -> bool {
        let Some(envelope) = block.envelope.clone() else {
            return self.add_legacy_block(block);
        };

        if !self.streams.contains_key(&envelope.stream_key) {
            self.streams.insert(
                envelope.stream_key.clone(),
                StreamState {
                    generations: Vec::new(),
                },
            );
            self.order.push(envelope.stream_key.clone());
        }

        let stream = self
            .streams
            .get_mut(&envelope.stream_key)
            .expect("stream exists");
        if stream.generations.is_empty() {
            stream.start_generation();
        }
        let signature = block_signature(block.record_type, &block.rows);
        let segment = Segment {
            index: envelope.segment_index,
            rows: block.rows,
            signature,
        };
        let current = stream.generations.last_mut().expect("generation exists");
        if let Some(existing) = current.segments.get(&segment.index) {
            if existing.signature == segment.signature {
                return false;
            }
            stream.start_generation();
        }
        stream
            .generations
            .last_mut()
            .expect("generation exists")
            .segments
            .insert(segment.index, segment);
        self.rows_dirty = true;
        true
    }

    pub fn rows(&mut self) -> Vec<ParsedRow> {
        self.refresh_rows();
        self.rows_cache.clone()
    }

    fn refresh_rows(&mut self) {
        if !self.rows_dirty {
            return;
        }
        let mut rows = Vec::new();
        for key in self.order.clone() {
            if key == "__legacy__" {
                rows.extend(self.legacy_rows.clone());
                continue;
            }
            if let Some(stream) = self.streams.get(&key).cloned() {
                rows.extend(self.assemble_stream(&key, &stream));
            }
        }
        self.rows_cache = rows;
        self.rows_dirty = false;
    }

    fn add_legacy_block(&mut self, block: ParsedBlock) -> bool {
        let signature = block_signature(block.record_type, &block.rows);
        if self.legacy_blocks.contains(&signature) {
            return false;
        }
        if self.legacy_rows.is_empty() {
            self.order.push("__legacy__".to_string());
        }
        self.legacy_blocks.insert(signature);
        self.legacy_rows.extend(block.rows);
        self.rows_dirty = true;
        true
    }

    fn assemble_stream(&mut self, key: &str, stream: &StreamState) -> Vec<ParsedRow> {
        let mut result_rows = Vec::new();
        let mut result_max_segment: Option<u32> = None;

        for generation in &stream.generations {
            if generation.segments.is_empty() {
                continue;
            }
            let generation_rows = rows_with_generation(generation);
            let generation_min = *generation.segments.keys().next().expect("nonempty");
            let generation_max = *generation.segments.keys().next_back().expect("nonempty");

            if result_rows.is_empty() {
                result_rows = generation_rows;
                result_max_segment = Some(generation_max);
                continue;
            }

            if generation_min == 0 {
                if result_max_segment.is_none_or(|max| generation_max >= max) {
                    result_rows = generation_rows;
                    result_max_segment = Some(generation_max);
                    continue;
                }
                if let Some(merged) = partial_snapshot_merge(&generation_rows, &result_rows) {
                    result_rows = merged;
                } else {
                    self.warn_generation(
                        "ambiguous_snapshot_merge",
                        format!("{key}: partial snapshot cannot be merged safely"),
                        generation,
                    );
                }
                continue;
            }

            if result_max_segment.is_some_and(|max| generation_min > max) {
                result_rows.extend(generation_rows);
                result_max_segment = Some(generation_max);
                continue;
            }

            self.warn_generation(
                "ambiguous_snapshot_merge",
                format!("{key}: non-zero snapshot reset cannot be merged safely"),
                generation,
            );
        }

        result_rows
    }

    fn warn_generation(&mut self, code: &str, message: String, generation: &Generation) {
        let Some(row) = generation
            .segments
            .values()
            .next()
            .and_then(|segment| segment.rows.first())
        else {
            return;
        };
        let key = (
            code.to_string(),
            row.source.stream_key.clone().unwrap_or_default(),
            generation.index,
        );
        if self.warning_keys.contains(&key) {
            return;
        }
        self.warning_keys.insert(key);
        self.warnings.push(ParseWarning {
            code: code.to_string(),
            message,
            session: Some(row.source.session),
            line: Some(row.source.line),
            packet_index: Some(row.source.packet_index),
            view: Some(row.source.view.clone()),
        });
    }
}

#[cfg(any(windows, test))]
pub(crate) struct AssemblerUpdate {
    pub(crate) rows: Option<Vec<ParsedRow>>,
    pub(crate) new_rows: Vec<ParsedRow>,
}

impl StreamState {
    fn start_generation(&mut self) {
        self.generations.push(Generation {
            index: self.generations.len() as u32,
            segments: BTreeMap::new(),
        });
    }
}

