fn discover_target() -> DiagnosticTargetDiscovery {
    let mut warnings = Vec::new();
    let mut error = None;
    let pids = match find_process_pids("HTGame.exe") {
        Ok(pids) => pids,
        Err(err) => {
            error = Some(err.to_string());
            Vec::new()
        }
    };
    if pids.len() > 1 {
        warnings.push(format!(
            "multiple HTGame.exe processes found: {}",
            pids.len()
        ));
    }
    let mut candidates = Vec::new();
    for pid in pids {
        match candidate_ports(pid) {
            Ok(ports) => candidates.push(ProcessCandidate {
                pid,
                ports,
                error: None,
            }),
            Err(err) => candidates.push(ProcessCandidate {
                pid,
                ports: Vec::new(),
                error: Some(err.to_string()),
            }),
        }
    }
    let selected = candidates
        .iter()
        .find(|candidate| !candidate.ports.is_empty())
        .or_else(|| candidates.first());
    DiagnosticTargetDiscovery {
        exe: "HTGame.exe".to_string(),
        selected_pid: selected.map(|candidate| candidate.pid),
        selected_ports: selected
            .map(|candidate| candidate.ports.clone())
            .unwrap_or_default(),
        candidates,
        warnings,
        error,
    }
}
