use thing_host::HostRuntime;
use userland::{canon, fiat, find_by_kind, map, that, Value};
use uuid::Uuid;

fn main() {
    // Initialize host runtime
    let runtime = Box::leak(Box::new(HostRuntime::new()));
    userland::set_runtime(runtime);

    println!("Graph client started.");

    // Create "Hello" thing
    let hello_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"hello");
    let mut fields = map();
    fields.insert(canon::NAME, Value::text("Hello"));
    fiat(Some(hello_id), canon::WINDOW, fields);
    println!("Created Thing: Hello ({})", hello_id);

    // Create "World" thing
    let world_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"world");
    let mut fields = map();
    fields.insert(canon::NAME, Value::text("World"));
    fiat(Some(world_id), canon::WINDOW, fields);
    println!("Created Thing: World ({})", world_id);

    // Link them
    that(hello_id, canon::NEXT, world_id, 0);
    println!("Linked Hello -> World");

    // Query
    // Note: find_by_kind in Neo4jGraphStore currently returns all things
    let things = find_by_kind("window");
    println!("Found {} things:", things.len());
    for thing in things {
        println!(" - {:?} {:?}", thing.id, thing.fields);
    }
}
