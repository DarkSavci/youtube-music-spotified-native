use super::*;

fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("spotified-accounts-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

fn channel(id: &str, name: &str) -> Channel {
    Channel {
        id: id.into(),
        name: name.into(),
        handle: String::new(),
        avatar_url: String::new(),
    }
}

#[test]
fn a_profile_that_is_signed_in_already_becomes_the_first_account() {
    let root = scratch("legacy");
    std::fs::write(root.join(CREDENTIALS), r#"{"cookie":"c"}"#).expect("write");
    let store = AccountStore::load(&root);
    let account = store.list.active().expect("an account in use");
    assert!(account.legacy);
    assert_eq!(account.name, "Saved account");
    // It stays where it always was.
    assert_eq!(store.credentials(), root.join(CREDENTIALS));
    assert_eq!(store.database(), root.join(DATABASE));
    // And the list is on disk for the next launch to find.
    assert_eq!(AccountStore::load(&root).list, store.list);
}

#[test]
fn a_signed_out_profile_has_no_accounts_and_runs_in_its_root() {
    let root = scratch("guest");
    let store = AccountStore::load(&root);
    assert!(store.list.accounts.is_empty());
    assert_eq!(store.list.active, None);
    assert_eq!(store.database(), root.join(DATABASE));
}

#[test]
fn a_new_account_gets_a_folder_of_its_own_and_is_the_one_in_use() {
    let root = scratch("add");
    let mut store = AccountStore::load(&root);
    let account = new_account("New account");
    let credentials = store.credentials_for(&account).expect("a folder");
    assert_eq!(
        credentials,
        root.join("accounts").join(&account.id).join(CREDENTIALS)
    );
    store.list.add(account.clone());
    assert!(store.list.is_active(&account.id));
    assert_eq!(store.credentials(), credentials);
}

#[test]
fn ids_are_distinct_and_only_ever_hex_and_dashes() {
    let (one, other) = (new_id(), new_id());
    assert_ne!(one, other);
    assert!(valid_id(&one), "{one}");
    assert!(!valid_id("../../somewhere-else-entirely-on-the-disk"));
    assert!(!valid_id(""));
}

#[test]
fn an_id_edited_into_a_path_does_not_leave_the_profile() {
    let root = scratch("escape");
    let store = AccountStore::load(&root);
    let account = SavedAccount {
        id: "..\\..\\elsewhere".into(),
        ..SavedAccount::default()
    };
    assert_eq!(store.directory(Some(&account)), root);
}

#[test]
fn switching_is_only_to_an_account_that_is_saved() {
    let mut list = Accounts::default();
    let (ada, grace) = (new_account("Ada"), new_account("Grace"));
    list.add(ada.clone());
    list.add(grace.clone());
    assert!(list.is_active(&grace.id));
    assert!(list.activate(&ada.id));
    assert!(list.is_active(&ada.id));
    assert!(!list.activate("nobody"));
    assert!(list.is_active(&ada.id));
    let others: Vec<&str> = list.others().map(|account| account.name.as_str()).collect();
    assert_eq!(others, ["Grace"]);
}

#[test]
fn removing_the_account_in_use_leaves_nobody_signed_in() {
    let mut list = Accounts::default();
    let (ada, grace) = (new_account("Ada"), new_account("Grace"));
    list.add(ada.clone());
    list.add(grace.clone());
    assert!(list.remove(&grace.id).is_some());
    assert_eq!(list.active, None);
    assert_eq!(list.accounts.len(), 1);
    // Removing one that is not in use leaves the one that is.
    list.activate(&ada.id);
    list.add(grace.clone());
    list.activate(&ada.id);
    list.remove(&grace.id);
    assert!(list.is_active(&ada.id));
}

#[test]
fn an_account_takes_its_name_once_and_keeps_it() {
    let mut list = Accounts::default();
    list.add(new_account("New account"));
    assert!(list.set_name("Ada", "https://example.test/ada.jpg"));
    assert!(!list.set_name("Ada", "https://example.test/ada.jpg"));
    // The name a channel goes by does not replace the account's.
    list.select_channel("123");
    assert!(!list.set_name("Ada's band", "https://example.test/band.jpg"));
    let account = list.active().expect("an account in use");
    assert_eq!(account.name, "Ada");
    assert_eq!(account.avatar_url, "https://example.test/ada.jpg");
}

#[test]
fn the_channels_of_the_account_in_use_are_remembered() {
    let mut list = Accounts::default();
    list.add(new_account("Ada"));
    let channels = [channel("1", "Ada"), channel("2", "Ada's band")];
    assert!(list.set_channels(&channels));
    assert!(!list.set_channels(&channels));
    list.select_channel("2");
    let account = list.active().expect("an account in use");
    assert_eq!(account.channel_name(), Some("Ada's band"));
    // Signed out, there is nobody to remember them for.
    let mut nobody = Accounts::default();
    assert!(!nobody.set_channels(&channels));
}

#[test]
fn signing_an_account_out_deletes_its_session_and_keeps_its_history() {
    let root = scratch("remove");
    let mut store = AccountStore::load(&root);
    let account = new_account("Ada");
    let credentials = store.credentials_for(&account).expect("a folder");
    let folder = store.directory(Some(&account));
    for file in [CREDENTIALS, RESOLVER_COOKIES, ANSWERS, DATABASE] {
        std::fs::write(folder.join(file), "x").expect("write");
    }
    store.list.add(account.clone());
    store.remove(&account.id);
    assert!(!credentials.exists());
    assert!(!folder.join(RESOLVER_COOKIES).exists());
    assert!(!folder.join(ANSWERS).exists());
    assert!(folder.join(DATABASE).exists());
    assert!(AccountStore::load(&root).list.accounts.is_empty());
}

#[test]
fn the_list_follows_the_channel_the_credentials_name() {
    let root = scratch("channel");
    std::fs::write(
        root.join(CREDENTIALS),
        r#"{"cookie":"c","onBehalfOfUser":"123"}"#,
    )
    .expect("write");
    let store = AccountStore::load(&root);
    assert_eq!(store.list.active().map(|a| a.channel.as_str()), Some("123"));
}

#[test]
fn a_list_from_a_version_not_known_is_not_trusted() {
    let root = scratch("version");
    std::fs::write(
        root.join(FILE),
        r#"{"version":7,"active":"x","accounts":[{"id":"x"}]}"#,
    )
    .expect("write");
    assert!(AccountStore::load(&root).list.accounts.is_empty());
}

#[test]
fn the_electron_apps_list_reads_as_it_is() {
    let text = r#"{"version":1,"active":"a","accounts":[{"id":"a","name":"Ada","legacy":true,
        "channel":"2","avatarUrl":"u","channels":[{"id":"2","name":"Band","handle":"@band",
        "avatarUrl":"v","localId":"l"}]}]}"#;
    let list: Accounts = serde_json::from_str(text).expect("a list");
    let account = list.active().expect("an account in use");
    assert_eq!(account.avatar_url, "u");
    assert_eq!(account.channel_name(), Some("Band"));
}

#[test]
fn the_folder_of_the_account_in_use_is_there_before_the_core_needs_it() {
    let root = scratch("prepare");
    let mut store = AccountStore::load(&root);
    // Saved, but its folder has since gone.
    store.list.add(new_account("Ada"));
    let database = store.database();
    assert!(!database.parent().is_some_and(Path::exists));
    store.prepare();
    assert!(database.parent().is_some_and(Path::exists));
}

/// A profile from before channels had databases of their own: one account
/// in a folder of its own, acting as a channel, with one database.
fn from_before(name: &str) -> (PathBuf, String) {
    const ID: &str = "aaaaaaaa-0000-0000-0000-000000000001";
    let root = scratch(name);
    let list = format!(
        r#"{{"version":1,"active":"{ID}","accounts":[
        {{"id":"{ID}","name":"Ada","legacy":false,"avatarUrl":"","channel":"123",
          "channels":[{{"id":"","name":"Ada","handle":"@ada"}},
                      {{"id":"123","name":"Band","handle":"@band"}}]}}]}}"#
    );
    std::fs::write(root.join(FILE), list).expect("write");
    let folder = root.join("accounts").join(ID);
    std::fs::create_dir_all(&folder).expect("a folder");
    std::fs::write(
        folder.join(CREDENTIALS),
        r#"{"cookie":"c","onBehalfOfUser":"123"}"#,
    )
    .expect("write");
    std::fs::write(folder.join(DATABASE), "plays").expect("write");
    (root, ID.to_owned())
}

#[test]
fn an_account_from_before_has_its_database_moved_to_its_channel_once() {
    let (root, id) = from_before("split");
    let folder = root.join("accounts").join(&id);
    let mut store = AccountStore::load(&root);
    // Nothing moves until a core is about to start.
    assert_eq!(store.database(), folder.join(DATABASE));
    store.prepare();
    let account = store.list.active().expect("an account in use").clone();
    assert!(account.channel_databases);
    let local = local_id_of(&account, "123").expect("a folder name");
    let theirs = folder.join("channels").join(local).join(DATABASE);
    assert_eq!(store.database(), theirs);
    assert_eq!(std::fs::read_to_string(&theirs).expect("read"), "plays");
    assert!(!folder.join(DATABASE).exists());

    // The next launch finds it done, and the channel in the same folder.
    let mut again = AccountStore::load(&root);
    again.prepare();
    assert_eq!(again.database(), theirs);
    assert_eq!(std::fs::read_to_string(&theirs).expect("read"), "plays");
}

#[test]
fn the_accounts_own_channel_starts_afresh_once_the_database_is_its_channels() {
    let (root, id) = from_before("own-after");
    let folder = root.join("accounts").join(&id);
    let mut store = AccountStore::load(&root);
    store.prepare();
    store.list.select_channel("");
    store.prepare();
    // Its own database, which is not there yet: the core makes it.
    assert_eq!(store.database(), folder.join(DATABASE));
    assert!(!folder.join(DATABASE).exists());
    // And back as the channel, to what the channel listened to.
    store.list.select_channel("123");
    assert!(store.database().exists());
}

#[test]
fn the_channels_the_core_lists_keep_the_folders_they_have() {
    let (root, _) = from_before("keep-folders");
    let mut store = AccountStore::load(&root);
    store.prepare();
    let before = store.database();
    // The list from the core arrives, as it does after every start.
    let listed = [
        channel("", "Ada"),
        channel("123", "Band"),
        channel("456", "Other"),
    ];
    assert!(store.list.set_channels(&listed));
    assert_eq!(store.database(), before);
    // A channel new to the list has a folder name of its own at once.
    let account = store.list.active().expect("an account in use");
    let other = local_id_of(account, "456").expect("a folder name");
    assert!(valid_id(other));
    assert_ne!(Some(other), local_id_of(account, "123"));
}

#[test]
fn a_channel_named_only_by_the_credentials_gets_a_folder_before_the_core_starts() {
    // The list has never heard of the channel the sign-in acts as.
    let root = scratch("unlisted-channel");
    std::fs::write(
        root.join(CREDENTIALS),
        r#"{"cookie":"c","onBehalfOfUser":"789"}"#,
    )
    .expect("write");
    std::fs::write(root.join(DATABASE), "plays").expect("write");
    let mut store = AccountStore::load(&root);
    store.prepare();
    let account = store.list.active().expect("an account in use").clone();
    let local = local_id_of(&account, "789").expect("a folder name");
    let theirs = root.join("channels").join(local).join(DATABASE);
    assert_eq!(store.database(), theirs);
    assert_eq!(std::fs::read_to_string(theirs).expect("read"), "plays");
    // When the core then names it, it keeps that folder.
    let mut list = store.list.clone();
    list.set_channels(&[channel("", "Ada"), channel("789", "Band")]);
    let named = list.active().expect("an account in use");
    assert_eq!(local_id_of(named, "789"), Some(local));
}

#[test]
fn signing_out_forgets_what_youtube_answered_each_channel() {
    let (root, id) = from_before("answers");
    let folder = root.join("accounts").join(&id);
    let mut store = AccountStore::load(&root);
    store.prepare();
    let theirs = store.database().parent().expect("a folder").to_path_buf();
    std::fs::write(folder.join(ANSWERS), "a").expect("write");
    std::fs::write(theirs.join(ANSWERS), "b").expect("write");
    store.remove(&id);
    assert!(!folder.join(ANSWERS).exists());
    assert!(!theirs.join(ANSWERS).exists());
    // The listening itself stays.
    assert!(theirs.join(DATABASE).exists());
}

#[test]
fn the_electron_apps_list_gives_each_channel_the_folder_it_had() {
    let list = r#"{"version":1,"active":null,"accounts":[
        {"id":"aaaaaaaa-0000-0000-0000-000000000001","name":"Ada","legacy":false,"channel":"",
         "channels":[{"id":"123","name":"Band","handle":"@band",
                      "localId":"bbbbbbbb-0000-0000-0000-00000000000b"}]}]}"#;
    let list: Accounts = serde_json::from_str(list).expect("a list");
    let account = &list.accounts[0];
    assert_eq!(
        local_id_of(account, "123"),
        Some("bbbbbbbb-0000-0000-0000-00000000000b")
    );
    // A list that says nothing of it is from before each had its own.
    assert!(!account.channel_databases);
}
