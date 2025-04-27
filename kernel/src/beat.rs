use crate::space::Space;
use crate::thing::Fact;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::Any;
use uuid::Uuid;

/// A tickable component that produces effects on the graph
pub trait Beat: Any {
    /// Called each frame/tick.
    /// `self_id` is the Thing's UUID.
    /// `space` provides read-only access to the graph.
    fn beat(&mut self, self_id: Uuid, space: &Space) -> Vec<Fact>;
}

/// Invoke `beat()` on all Things in the graph that implement `Beat`
pub fn beat_all_things(space: &mut Space) -> Vec<Fact> {
    let mut all_facts = Vec::new();

    // Collect (ID, Box<dyn Beat>) pairs by cloning the boxed trait object if possible
    let thing_ids: Vec<Uuid> = space.things.keys().cloned().collect();

    for id in thing_ids {
        if let Some(thing) = space.things.get_mut(&id) {
            let data = thing.data.as_any_mut();
            if let Some(beater) = thing.data.as_any_mut().downcast_mut::<dyn Beat>() {
                let new_facts = beater.beat(id, &*space);
                all_facts.extend(new_facts);
            }
        }
    }

    all_facts
}
