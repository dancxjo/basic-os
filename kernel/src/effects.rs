use alloc::{boxed::Box, vec::Vec};

use crate::{pattern::Pattern, space::Space, thing::Fact};

type Reaction = Box<dyn FnMut(&Fact) -> Vec<Fact>>;

pub struct Effect {
    pub pattern: Pattern,
    pub handler: Reaction,
}

impl Space {
    pub fn when<F>(&mut self, pattern: Pattern, handler: F)
    where
        F: 'static + FnMut(&Fact) -> Vec<Fact>,
    {
        self.effects.push(Effect {
            pattern,
            handler: Box::new(handler),
        });
    }
}
