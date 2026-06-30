#[test]
fn profile_name_validation_rejects_unsafe_and_duplicate_names() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();

    store.create_profile("Player_1").unwrap();

    assert!(store.create_profile("Player 1").is_err());
    assert!(store.create_profile("../bad").is_err());
    assert!(store.create_profile("player_1").is_err());
    assert!(store.create_profile("CON").is_err());
    assert!(store.create_profile("LPT1").is_err());
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
