#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use userland::prelude::*;
use userland::{canon, graph, Symbol};
use uuid::Uuid;

const SMOKE_KIND: Symbol = canon::canon(b'G', b'S', b'M');
const NODE_NAME: &str = "graph-smoke-node";

struct GraphSmoke {
    node: Uuid,
    watch: WatchId,
    seen_events: bool,
}

impl App for GraphSmoke {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        let node_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, NODE_NAME.as_bytes());
        let mut fields = graph::map();
        fields.insert(canon::NAME, graph::Value::Text(String::from(NODE_NAME)));
        fields.insert(canon::STATUS, graph::Value::Text(String::from("created")));

        let declared = graph::fiat(Some(node_id), SMOKE_KIND, fields);

        let mut pattern = graph::NodePattern::default();
        pattern.labels.push(SMOKE_KIND);
        let matches = graph::get_nodes(pattern);
        println!(
            "graph-smoke: declared {} and observed {} matches",
            declared,
            matches.len()
        );

        let watch = ctx.watch_graph(ThingFilter {
            kind: Some(SMOKE_KIND),
            id: None,
        });

        GraphSmoke {
            node: declared,
            watch,
            seen_events: false,
        }
    }

    fn tick(&mut self, _ctx: &mut AppContext<'_>, tick: u64) {
        if !self.seen_events && tick % 8 == 0 {
            let mut props = graph::map();
            props.insert(canon::STATUS, graph::Value::Text(String::from("ticking")));
            let _ = graph::set_props(graph::GraphPropsRequest {
                node: self.node,
                props,
            });
        }
    }

    fn on_event(&mut self, _ctx: &mut AppContext<'_>, ev: AppEvent) {
        if let AppEvent::Thing { watch, thing } = ev {
            if watch == self.watch && thing.id == self.node {
                println!("graph-smoke: observed revision {}", thing.revision);
                self.seen_events = true;
            }
        }
    }
}

app_main!(GraphSmoke);
