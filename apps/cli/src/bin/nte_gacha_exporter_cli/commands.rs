fn doctor() -> CliResult<()> {
    let report = capture_doctor(EXE_NAME).map_err(CliError::from_error)?;
    println!(
        "Windows: {}",
        if report.windows { "ok" } else { "unavailable" }
    );
    println!("Admin: {}", if report.admin { "ok" } else { "required" });
    println!("Process: {} {:?}", report.exe, report.pid);
    println!("Ports: {:?}", report.ports);
    for note in &report.notes {
        println!("{note}");
    }
    if report.windows && report.admin && report.pid.is_some() && !report.ports.is_empty() {
        Ok(())
    } else {
        Err(CliError::new(3, "capture environment is not ready"))
    }
}

fn maps_build(args: MapBuildArgs) -> CliResult<()> {
    let assets_root =
        find_assets_root(args.assets_root.as_deref()).map_err(CliError::from_error)?;
    let out_dir = args.out_dir.unwrap_or_else(default_maps_output_dir);
    fs::create_dir_all(&out_dir).map_err(CliError::from_error)?;

    for build in
        build_asset_maps(&assets_root, args.locale.as_deref()).map_err(CliError::from_error)?
    {
        let out = out_dir.join(format!("{}.json", build.locale));
        let bytes = serde_json::to_vec_pretty(&build.map).map_err(CliError::from_error)?;
        fs::write(&out, bytes).map_err(CliError::from_error)?;
        println!(
            "{}: items={} pools={} labels={}",
            build.locale, build.item_count, build.pool_count, build.label_count
        );
    }
    Ok(())
}

fn assets_pack_build(args: AssetsPackBuildArgs) -> CliResult<()> {
    let assets_root =
        find_assets_root(args.assets_root.as_deref()).map_err(CliError::from_error)?;
    let maps_dir = args.maps_dir.unwrap_or_else(default_maps_output_dir);
    let build = build_assets_pack(&AssetPackBuildOptions {
        assets_root,
        maps_dir,
        out_path: args.out,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        webp_quality: args.quality,
    })
    .map_err(CliError::from_error)?;
    println!("assets={}", build.manifest.file_count);
    println!("map_hash={}", build.manifest.map_hash);
    println!("source_commit={}", build.manifest.source_commit);
    println!("pack={}", build.out_path.display());
    Ok(())
}

fn default_maps_output_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crates/nte-assets/resources/maps")
}

fn progress_callback(
    locale: &str,
    verbose: bool,
) -> Arc<dyn Fn(CaptureProgress) + Send + Sync + 'static> {
    let builder = Arc::new(Mutex::new(CaptureRecordBuilder::new(locale).ok()));
    Arc::new(move |progress: CaptureProgress| {
        if verbose && !progress.new_rows.is_empty() {
            if let Ok(mut guard) = builder.lock() {
                if let Some(builder) = guard.as_mut() {
                    for record in builder.build_records(&progress.new_rows) {
                        println!("{}", record.value);
                    }
                }
            }
        }
        eprint!(
            "\rrecords={} packets={} decoded={} dropped={} duplicates={}",
            progress.row_count,
            progress.counters.packets_seen,
            progress.counters.decoded_packets,
            progress.counters.dropped_packets,
            progress.counters.duplicate_packets
        );
    })
}

fn print_auto_status(status: AutoPageStatus) {
    eprintln!(
        "auto_page: {} pool={} page={}/{} {}",
        status.message,
        status.pool.unwrap_or_else(|| "-".to_string()),
        status
            .current_page
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
        status
            .total_pages
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
        status.technical_detail
    );
}
