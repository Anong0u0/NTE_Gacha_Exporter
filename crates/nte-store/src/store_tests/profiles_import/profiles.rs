#[test]
fn profile_name_validation_rejects_unsafe_and_duplicate_names() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();

    store.create_profile("Player_1").unwrap();

    let unicode = store.create_profile("玩家 一號✨").unwrap();
    assert_eq!(unicode.name, "玩家 一號✨");
    assert!(tmp.path().join("data/profiles/玩家 一號✨").is_dir());

    for name in [
        "../bad",
        "bad/name",
        "bad\\name",
        "bad:name",
        ".",
        "name.",
        "CON",
        "CON.txt",
        "LPT1",
    ] {
        assert!(
            matches!(
                store.create_profile(name),
                Err(GuiError::Profile(
                    ProfileError::NameUnsafe | ProfileError::NameReserved
                ))
            ),
            "unexpectedly accepted {name:?}"
        );
    }
    assert!(matches!(
        store.create_profile("player_1"),
        Err(GuiError::Profile(ProfileError::AlreadyExists(_)))
    ));
    assert!(matches!(
        store.create_profile(&"😀".repeat(128)),
        Err(GuiError::Profile(ProfileError::NameTooLong))
    ));
}

#[test]
fn profile_names_normalize_and_case_fold_before_duplicate_checks() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();

    let normalized = store.create_profile("Cafe\u{301}").unwrap();
    assert_eq!(normalized.name, "Café");
    assert!(tmp.path().join("data/profiles/Café").is_dir());
    assert!(matches!(
        store.create_profile("CAFÉ"),
        Err(GuiError::Profile(ProfileError::AlreadyExists(_)))
    ));

    store.create_profile("Straße").unwrap();
    assert!(matches!(
        store.create_profile("STRASSE"),
        Err(GuiError::Profile(ProfileError::AlreadyExists(_)))
    ));
}

#[test]
fn profile_name_accepts_windows_component_limit_and_cleans_staging() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let max_name = "x".repeat(255);

    assert_eq!(store.create_profile(&max_name).unwrap().name, max_name);

    let stale = tmp.path().join("data/.profile-staging/create-stale");
    std::fs::create_dir_all(&stale).unwrap();
    std::fs::write(stale.join("profile.json"), "{}").unwrap();
    std::fs::write(stale.join("records.json"), "{}").unwrap();
    drop(store);

    JsonStore::open(tmp.path()).unwrap();
    assert!(!stale.exists());
}

#[test]
fn profile_rename_updates_files_and_active_settings() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    store.create_profile("Player_1").unwrap();
    store.set_active_profile("Player_1").unwrap();

    let renamed = store.rename_profile("Player_1", "Player_2").unwrap();
    let settings = store.settings().unwrap();
    let profiles = store.list_profiles().unwrap();

    assert_eq!(renamed.name, "Player_2");
    assert!(renamed.active);
    assert_eq!(settings.active_profile, "Player_2");
    assert!(
        !tmp.path()
            .join("data/profiles/Player_1/profile.json")
            .exists()
    );
    assert!(
        tmp.path()
            .join("data/profiles/Player_2/profile.json")
            .exists()
    );
    assert!(profiles.iter().any(|profile| profile.name == "Player_2"));
    assert!(!profiles.iter().any(|profile| profile.name == "Player_1"));
}

#[test]
fn profile_rename_rejects_duplicates_case_insensitively() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    store.create_profile("Player_1").unwrap();
    store.create_profile("Player_2").unwrap();

    assert!(store.rename_profile("Player_1", "player_2").is_err());
    assert!(
        tmp.path()
            .join("data/profiles/Player_1/profile.json")
            .exists()
    );
    assert!(
        tmp.path()
            .join("data/profiles/Player_2/profile.json")
            .exists()
    );
}

#[test]
fn profile_delete_active_switches_to_remaining_profile() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    store.create_profile("Player_1").unwrap();
    store.set_active_profile("Player_1").unwrap();

    let settings = store.delete_profile("Player_1").unwrap();
    let profiles = store.list_profiles().unwrap();

    assert_eq!(settings.active_profile, "default");
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].name, "default");
    assert!(
        !tmp.path()
            .join("data/profiles/Player_1/profile.json")
            .exists()
    );
}

#[test]
fn deleted_default_profile_is_not_recreated_on_reopen() {
    let tmp = tempfile::tempdir().unwrap();
    {
        let store = JsonStore::open(tmp.path()).unwrap();
        store.create_profile("Player_1").unwrap();
        store.set_active_profile("Player_1").unwrap();
        store.delete_profile("default").unwrap();
    }

    let store = JsonStore::open(tmp.path()).unwrap();
    let profiles = store.list_profiles().unwrap();

    assert_eq!(store.settings().unwrap().active_profile, "Player_1");
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].name, "Player_1");
    assert!(profiles[0].active);
    assert!(!tmp.path().join("data/profiles/default").exists());
}

#[test]
fn profile_delete_rejects_last_profile_and_unknown_files() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();

    assert!(store.delete_profile("default").is_err());

    store.create_profile("Player_1").unwrap();
    std::fs::write(tmp.path().join("data/profiles/Player_1/extra.json"), "{}").unwrap();
    assert!(store.delete_profile("Player_1").is_err());
    assert!(
        tmp.path()
            .join("data/profiles/Player_1/profile.json")
            .exists()
    );
    assert!(
        tmp.path()
            .join("data/profiles/Player_1/extra.json")
            .exists()
    );
}
