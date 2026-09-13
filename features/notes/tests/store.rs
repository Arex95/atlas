//! Integration tests for personal notes, against a real `SQLite`
//! database.
//!
//! The property that matters most is the one this crate exists for: a note
//! belongs to exactly one developer, and there is no call shape that
//! reaches somebody else's.

use atlas_notes::api::{Note, NoteStore, NotesError, SqlitePool, run_migrations};

const ALICE: &str = "01OWNERALICE";
const BOB: &str = "01OWNERBOB";

async fn store() -> NoteStore {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    run_migrations(&pool).await.unwrap();
    NoteStore::new(pool)
}

#[tokio::test]
async fn a_note_round_trips() {
    let store = store().await;
    let written = store
        .write(ALICE, "todo", "ship the notes crate")
        .await
        .unwrap();

    assert_eq!(written.owner_id, ALICE);
    assert_eq!(written.name, "todo");
    assert_eq!(written.body, "ship the notes crate");

    let read = store.read(ALICE, "todo").await.unwrap();
    assert_eq!(read, written);
}

#[tokio::test]
async fn writing_the_same_name_replaces_the_body_and_keeps_the_note() {
    let store = store().await;
    let first = store.write(ALICE, "todo", "one").await.unwrap();
    let second = store.write(ALICE, "todo", "two").await.unwrap();

    assert_eq!(second.body, "two");
    // The same note, rewritten — not a second one.
    assert_eq!(store.list(ALICE).await.unwrap().len(), 1);
    assert_eq!(
        second.created_at, first.created_at,
        "a rewrite reset the note's age"
    );
    assert!(second.updated_at >= first.updated_at);
}

/// The property the whole crate exists for.
#[tokio::test]
async fn one_developers_notes_are_unreachable_to_another() {
    let store = store().await;
    store
        .write(ALICE, "salary", "asking for more")
        .await
        .unwrap();

    // Not found rather than forbidden: distinguishing them would say
    // whether a name exists in somebody else's notes.
    assert_eq!(
        store.read(BOB, "salary").await.unwrap_err(),
        NotesError::NotFound
    );
    assert_eq!(store.list(BOB).await.unwrap(), vec![]);
    assert_eq!(
        store.delete(BOB, "salary").await.unwrap_err(),
        NotesError::NotFound
    );

    // And Alice's is untouched by all of that.
    assert_eq!(
        store.read(ALICE, "salary").await.unwrap().body,
        "asking for more"
    );
}

#[tokio::test]
async fn the_same_name_under_two_owners_is_two_notes() {
    let store = store().await;
    store.write(ALICE, "todo", "alice's").await.unwrap();
    store.write(BOB, "todo", "bob's").await.unwrap();

    assert_eq!(store.read(ALICE, "todo").await.unwrap().body, "alice's");
    assert_eq!(store.read(BOB, "todo").await.unwrap().body, "bob's");
}

#[tokio::test]
async fn listing_puts_the_most_recently_written_first() {
    let store = store().await;
    store.write(ALICE, "old", "1").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    store.write(ALICE, "new", "2").await.unwrap();

    let names: Vec<String> = store
        .list(ALICE)
        .await
        .unwrap()
        .into_iter()
        .map(|n| n.name)
        .collect();
    assert_eq!(names, vec!["new", "old"]);
}

#[tokio::test]
async fn deleting_something_that_was_never_there_is_reported() {
    // Silently succeeding would hide the usual cause: a typo in the
    // name.
    let store = store().await;
    assert_eq!(
        store.delete(ALICE, "never-existed").await.unwrap_err(),
        NotesError::NotFound
    );
}

#[tokio::test]
async fn a_name_is_trimmed_so_it_addresses_one_note() {
    let store = store().await;
    store.write(ALICE, "  todo  ", "body").await.unwrap();
    // Otherwise " todo" and "todo" would be two notes a developer
    // cannot tell apart on screen.
    assert_eq!(store.read(ALICE, "todo").await.unwrap().body, "body");
    assert_eq!(store.list(ALICE).await.unwrap().len(), 1);
}

#[tokio::test]
async fn an_empty_or_oversized_name_is_refused() {
    let store = store().await;
    assert_eq!(
        store.write(ALICE, "   ", "body").await.unwrap_err(),
        NotesError::EmptyName
    );
    // Counted in characters: an accented name is not longer than the
    // same name in ASCII to whoever typed it.
    let long = "é".repeat(201);
    assert!(matches!(
        store.write(ALICE, &long, "body").await.unwrap_err(),
        NotesError::NameTooLong(_)
    ));
    assert!(store.write(ALICE, &"é".repeat(200), "body").await.is_ok());
}

#[tokio::test]
async fn an_empty_body_is_allowed() {
    // Clearing a scratchpad without deleting it is a normal thing to
    // want.
    let store = store().await;
    assert_eq!(store.write(ALICE, "scratchpad", "").await.unwrap().body, "");
}

#[tokio::test]
async fn list_since_pages_forward_from_a_cursor() {
    let store = store().await;
    store.write(ALICE, "first", "1").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    let second = store.write(ALICE, "second", "2").await.unwrap();

    assert_eq!(store.list_since(ALICE, None).await.unwrap().len(), 2);
    let after_first = store
        .list_since(
            ALICE,
            Some(second.updated_at - chrono::Duration::milliseconds(500)),
        )
        .await
        .unwrap();
    assert_eq!(after_first.len(), 1);
    assert_eq!(after_first[0].name, "second");

    // Strictly after: a cursor at the newest row returns nothing, so a
    // poll loop does not re-deliver what it just saw.
    assert!(
        store
            .list_since(ALICE, Some(second.updated_at))
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn a_synced_note_is_stored_under_the_authenticated_owner() {
    let store = store().await;
    let incoming = Note {
        id: "from-another-machine".to_owned(),
        // A client claiming somebody else's notes.
        owner_id: BOB.to_owned(),
        name: "todo".to_owned(),
        body: "pushed".to_owned(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let applied = store.upsert_for_sync(incoming, ALICE).await.unwrap().0;
    assert_eq!(
        applied.owner_id, ALICE,
        "a client wrote into another's notes"
    );
    assert_eq!(store.list(BOB).await.unwrap(), vec![]);
}

#[tokio::test]
async fn an_older_incoming_note_does_not_overwrite_a_newer_one() {
    let store = store().await;
    let mine = store.write(ALICE, "todo", "mine, newer").await.unwrap();

    let stale = Note {
        id: "elsewhere".to_owned(),
        owner_id: ALICE.to_owned(),
        name: "todo".to_owned(),
        body: "theirs, older".to_owned(),
        created_at: mine.created_at,
        updated_at: mine.updated_at - chrono::Duration::seconds(60),
    };

    let applied = store.upsert_for_sync(stale, ALICE).await.unwrap().0;
    assert_eq!(applied.body, "mine, newer");
}

#[tokio::test]
async fn a_newer_incoming_note_wins_even_with_a_different_id() {
    // Two machines that wrote the same note independently produced
    // different ids for what the developer considers one note, so
    // last-write-wins compares on the name.
    let store = store().await;
    let mine = store.write(ALICE, "todo", "mine").await.unwrap();

    let newer = Note {
        id: "a-completely-different-id".to_owned(),
        owner_id: ALICE.to_owned(),
        name: "todo".to_owned(),
        body: "theirs, newer".to_owned(),
        created_at: mine.created_at,
        updated_at: mine.updated_at + chrono::Duration::seconds(60),
    };

    let applied = store.upsert_for_sync(newer, ALICE).await.unwrap().0;
    assert_eq!(applied.body, "theirs, newer");
    assert_eq!(
        store.list(ALICE).await.unwrap().len(),
        1,
        "it made a second note"
    );
}

/// A re-push of the same row reports that nothing moved.
///
/// This is what stops `live` from spinning: the sync client pushes
/// everything it has on every pass, so in a steady state the remote
/// receives rows it already holds. If the store called that a change,
/// the server announced one, the subscriber answered with a pass, and
/// the pass pushed again — measured at 256 events a second between two
/// machines with nobody touching either.
#[tokio::test]
async fn re_pushing_an_unchanged_note_reports_no_movement() {
    let store = store().await;
    let note = store.write(ALICE, "todo", "the body").await.unwrap();

    let (_, changed) = store.upsert_for_sync(note.clone(), ALICE).await.unwrap();
    assert!(!changed, "an unchanged row was reported as a change");

    // And a genuinely newer one still is.
    let newer = Note {
        updated_at: note.updated_at + chrono::Duration::seconds(30),
        body: "a newer body".to_owned(),
        ..note
    };
    let (applied, changed) = store.upsert_for_sync(newer, ALICE).await.unwrap();
    assert!(changed, "a newer row was reported as no change");
    assert_eq!(applied.body, "a newer body");
}
