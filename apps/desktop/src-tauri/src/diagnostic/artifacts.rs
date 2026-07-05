fn support_paths(root: &Path, session_id: &str) -> SupportPaths {
    let support_dir = root.join("data").join("support");
    let prefix = format!("diagnostic-{session_id}");
    SupportPaths {
        support_dir: support_dir.clone(),
        zip_path: support_dir.join(format!("{prefix}.zip")),
        diagnostic_json: support_dir.join(format!("{prefix}.diagnostic.json")),
        internal_raw: support_dir.join(format!("{prefix}.internal.raw.jsonl")),
        dropped_samples: support_dir.join(format!("{prefix}.internal-dropped-samples.jsonl")),
        external_etl: support_dir.join(format!("{prefix}.external.etl")),
        external_pcapng: support_dir.join(format!("{prefix}.external.pcapng")),
        external_stdout: support_dir.join(format!("{prefix}.external.stdout.txt")),
        external_stderr: support_dir.join(format!("{prefix}.external.stderr.txt")),
        counters_json: support_dir.join(format!("{prefix}.pktmon-counters.json")),
        counters_txt: support_dir.join(format!("{prefix}.pktmon-counters.txt")),
        external_commands: support_dir.join(format!("{prefix}.external-commands.json")),
    }
}

impl SupportPaths {
    fn clone_for_thread(&self) -> Self {
        Self {
            support_dir: self.support_dir.clone(),
            zip_path: self.zip_path.clone(),
            diagnostic_json: self.diagnostic_json.clone(),
            internal_raw: self.internal_raw.clone(),
            dropped_samples: self.dropped_samples.clone(),
            external_etl: self.external_etl.clone(),
            external_pcapng: self.external_pcapng.clone(),
            external_stdout: self.external_stdout.clone(),
            external_stderr: self.external_stderr.clone(),
            counters_json: self.counters_json.clone(),
            counters_txt: self.counters_txt.clone(),
            external_commands: self.external_commands.clone(),
        }
    }
}

fn collect_artifacts(paths: &SupportPaths) -> Vec<DiagnosticArtifact> {
    artifact_specs(paths)
        .into_iter()
        .map(|(name, path)| artifact(name, path))
        .collect()
}

fn artifact(name: &str, path: &Path) -> DiagnosticArtifact {
    let metadata = fs::metadata(path).ok();
    DiagnosticArtifact {
        name: name.to_string(),
        path: Some(path.to_string_lossy().to_string()),
        exists: metadata.as_ref().is_some_and(|metadata| metadata.is_file()),
        size_bytes: metadata.map(|metadata| metadata.len()),
    }
}

fn artifact_specs(paths: &SupportPaths) -> Vec<(&'static str, &Path)> {
    vec![
        ("diagnostic.json", paths.diagnostic_json.as_path()),
        ("internal.raw.jsonl", paths.internal_raw.as_path()),
        (
            "internal-dropped-samples.jsonl",
            paths.dropped_samples.as_path(),
        ),
        ("external.etl", paths.external_etl.as_path()),
        ("external.pcapng", paths.external_pcapng.as_path()),
        ("pktmon-counters.json", paths.counters_json.as_path()),
        ("pktmon-counters.txt", paths.counters_txt.as_path()),
        ("external.stdout.txt", paths.external_stdout.as_path()),
        ("external.stderr.txt", paths.external_stderr.as_path()),
        ("external-commands.json", paths.external_commands.as_path()),
    ]
}

fn write_support_zip(paths: &SupportPaths) -> anyhow::Result<()> {
    let file = File::create(&paths.zip_path)?;
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, path) in artifact_specs(paths) {
        if !path.is_file() {
            continue;
        }
        zip.start_file(name, options)?;
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        zip.write_all(&buffer)?;
    }
    zip.finish()?;
    Ok(())
}

fn rotate_support_zips(support_dir: &Path, preserve: &Path) -> anyhow::Result<()> {
    let mut zips = fs::read_dir(support_dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("diagnostic-") && name.ends_with(".zip"))
        })
        .collect::<Vec<_>>();
    zips.sort();
    for path in zips {
        if path != preserve {
            let _ = fs::remove_file(path);
        }
    }
    Ok(())
}

fn cleanup_artifact_files(paths: &SupportPaths) {
    for (_, path) in artifact_specs(paths) {
        let _ = fs::remove_file(path);
    }
}

fn cleanup_diagnostic_staging(paths: &SupportPaths) {
    cleanup_artifact_files(paths);
    let _ = fs::remove_file(&paths.zip_path);
    let staging_dir = external_pktmon_staging_dir();
    let staging_paths = external_staging_paths(paths, &staging_dir);
    let _ = fs::remove_file(staging_paths.etl);
    let _ = fs::remove_file(staging_paths.pcapng);
}
