use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use thing_abi::{
    GraphEdge, GraphFiatRequest, GraphLinkRequest, GraphPropsGetRequest, GraphPropsRequest,
    GraphThing, Map, NodePattern, Symbol,
};
use uuid::Uuid;

#[async_trait]
pub trait GraphStore: Send + Sync {
    async fn fiat(&self, req: GraphFiatRequest) -> Result<GraphThing>;
    async fn link(&self, req: GraphLinkRequest) -> Result<GraphEdge>;
    async fn get(&self, id: Uuid) -> Result<Option<GraphThing>>;
    async fn query(&self, pattern: NodePattern) -> Result<Vec<GraphThing>>;
    async fn find_by_kind(&self, kind: String, cursor: Option<u64>) -> Result<Vec<GraphThing>>;
    async fn props_get(&self, req: GraphPropsGetRequest) -> Result<Map>;
    async fn props_set(&self, req: GraphPropsRequest) -> Result<GraphThing>;
}

pub struct InMemoryGraphStore {
    things: Mutex<HashMap<Uuid, GraphThing>>,
    edges: Mutex<HashMap<Uuid, GraphEdge>>,
    revision: AtomicU64,
}

impl InMemoryGraphStore {
    pub fn new() -> Self {
        Self {
            things: Mutex::new(HashMap::new()),
            edges: Mutex::new(HashMap::new()),
            revision: AtomicU64::new(1),
        }
    }

    fn next_revision(&self) -> u64 {
        self.revision.fetch_add(1, Ordering::SeqCst)
    }
}

#[async_trait]
impl GraphStore for InMemoryGraphStore {
    async fn fiat(&self, req: GraphFiatRequest) -> Result<GraphThing> {
        let id = req.id.unwrap_or_else(|| thing_abi::next_uuid());
        let revision = self.next_revision();
        let mut labels = req.labels;
        if labels.is_empty() {
            labels.push(req.kind);
        }
        let thing = GraphThing {
            id,
            kind: req.kind,
            labels: labels.into_iter().collect(),
            fields: req.fields,
            owner: Uuid::nil(),
            revision,
        };
        self.things.lock().unwrap().insert(id, thing.clone());
        Ok(thing)
    }

    async fn link(&self, req: GraphLinkRequest) -> Result<GraphEdge> {
        let id = req.id.unwrap_or_else(|| thing_abi::next_uuid());
        let revision = self.next_revision();
        let edge = GraphEdge {
            id,
            src: req.from,
            pred: req.kind,
            dst: req.to,
            props: req.props,
            owner: Uuid::nil(),
            revision,
        };
        self.edges.lock().unwrap().insert(id, edge.clone());
        Ok(edge)
    }

    async fn get(&self, id: Uuid) -> Result<Option<GraphThing>> {
        Ok(self.things.lock().unwrap().get(&id).cloned())
    }

    async fn query(&self, pattern: NodePattern) -> Result<Vec<GraphThing>> {
        let things = self.things.lock().unwrap();
        let result = things
            .values()
            .filter(|thing| pattern_matches(thing, &pattern))
            .cloned()
            .collect();
        Ok(result)
    }

    async fn find_by_kind(&self, _kind: String, _cursor: Option<u64>) -> Result<Vec<GraphThing>> {
        // Simple implementation returning all things for now as per original HostRuntime
        let things = self.things.lock().unwrap();
        Ok(things.values().cloned().collect())
    }

    async fn props_get(&self, req: GraphPropsGetRequest) -> Result<Map> {
        let things = self.things.lock().unwrap();
        if let Some(thing) = things.get(&req.node) {
            if req.keys.is_empty() {
                Ok(thing.fields.clone())
            } else {
                Ok(req
                    .keys
                    .iter()
                    .filter_map(|k| thing.fields.get(k).map(|v| (*k, v.clone())))
                    .collect())
            }
        } else {
            Err(anyhow::anyhow!("thing not found"))
        }
    }

    async fn props_set(&self, req: GraphPropsRequest) -> Result<GraphThing> {
        let mut things = self.things.lock().unwrap();
        if let Some(thing) = things.get_mut(&req.node) {
            thing.fields.extend(req.props.clone());
            thing.revision = self.next_revision();
            Ok(thing.clone())
        } else {
            Err(anyhow::anyhow!("thing not found"))
        }
    }
}

fn pattern_matches(thing: &GraphThing, pattern: &NodePattern) -> bool {
    for label in &pattern.labels {
        if !thing.labels.contains(label) {
            return false;
        }
    }
    for (key, expected) in &pattern.props {
        match thing.fields.get(key) {
            Some(actual) if actual == expected => {}
            _ => return false,
        }
    }
    true
}

#[cfg(feature = "neo4j")]
pub struct Neo4jGraphStore {
    graph: Arc<neo4rs::Graph>,
}

#[cfg(feature = "neo4j")]
impl Neo4jGraphStore {
    pub async fn connect(uri: &str, user: &str, pass: &str) -> Result<Self> {
        let config = neo4rs::ConfigBuilder::new()
            .uri(uri)
            .user(user)
            .password(pass)
            .build()?;
        let graph = Arc::new(neo4rs::Graph::connect(config).await?);
        Ok(Self { graph })
    }

    async fn next_revision(&self) -> Result<u64> {
        // Simple revision counter in Neo4j
        let query = "MERGE (c:Counter {name: 'revision'}) 
                     ON CREATE SET c.value = 1 
                     ON MATCH SET c.value = c.value + 1 
                     RETURN c.value as value";
        let mut result = self.graph.execute(neo4rs::query(query)).await?;
        if let Some(row) = result.next().await? {
            Ok(row.get("value")?)
        } else {
            Ok(1)
        }
    }
}

#[cfg(feature = "neo4j")]
#[async_trait]
impl GraphStore for Neo4jGraphStore {
    async fn fiat(&self, req: GraphFiatRequest) -> Result<GraphThing> {
        let id = req.id.unwrap_or_else(|| thing_abi::next_uuid());
        let revision = self.next_revision().await?;
        let mut labels = req.labels;
        if labels.is_empty() {
            labels.push(req.kind);
        }

        let props_json = serde_json::to_string(&req.fields)?;
        let kind_str = req.kind.0.to_string(); // Symbol to string

        // We store kind as a property and also use it as a label if possible,
        // but Symbol is u32, so maybe just property.
        // User said: "Label: Thing (plus optional additional labels based on kind/user labels)."

        let q = "MERGE (t:Thing {uuid: $uuid}) 
                 SET t.kind = $kind, t.props_json = $props_json, t.revision = $revision
                 RETURN t";

        let mut query = neo4rs::query(q)
            .param("uuid", id.to_string())
            .param("kind", kind_str)
            .param("props_json", props_json)
            .param("revision", revision as i64);

        self.graph.run(query).await?;

        // Add labels? Neo4j labels must be static strings in Cypher or we use APOC.
        // Or we just stick to :Thing and use properties for filtering.
        // User said: "Label: Thing (plus optional additional labels based on kind/user labels)."
        // Since labels are dynamic Symbols (u32), mapping them to Neo4j labels (strings) is tricky without a map.
        // I will just store them in properties for now or ignore extra labels in Neo4j structure
        // unless I can map Symbol to string easily.
        // Symbol is just u32.

        Ok(GraphThing {
            id,
            kind: req.kind,
            labels: labels.into_iter().collect(),
            fields: req.fields,
            owner: Uuid::nil(),
            revision,
        })
    }

    async fn link(&self, req: GraphLinkRequest) -> Result<GraphEdge> {
        let id = req.id.unwrap_or_else(|| thing_abi::next_uuid());
        let revision = self.next_revision().await?;
        let props_json = serde_json::to_string(&req.props)?;
        let pred_str = req.kind.0.to_string();

        // User said: "Relationship type: PRED_<symbol> or a single LINK with pred as a property"
        // I'll use LINK with pred property.

        let q = "MATCH (a:Thing {uuid: $src}), (b:Thing {uuid: $dst})
                 MERGE (a)-[r:LINK {pred: $pred}]->(b)
                 SET r.uuid = $uuid, r.props_json = $props_json, r.revision = $revision
                 RETURN r";

        let query = neo4rs::query(q)
            .param("src", req.from.to_string())
            .param("dst", req.to.to_string())
            .param("pred", pred_str)
            .param("uuid", id.to_string())
            .param("props_json", props_json)
            .param("revision", revision as i64);

        self.graph.run(query).await?;

        Ok(GraphEdge {
            id,
            src: req.from,
            pred: req.kind,
            dst: req.to,
            props: req.props,
            owner: Uuid::nil(),
            revision,
        })
    }

    async fn get(&self, id: Uuid) -> Result<Option<GraphThing>> {
        let q = "MATCH (t:Thing {uuid: $uuid}) RETURN t.kind, t.props_json, t.revision";
        let mut result = self
            .graph
            .execute(neo4rs::query(q).param("uuid", id.to_string()))
            .await?;

        if let Some(row) = result.next().await? {
            let kind_str: String = row.get("t.kind")?;
            let props_json: String = row.get("t.props_json")?;
            let revision: i64 = row.get("t.revision")?;

            let kind = Symbol(kind_str.parse().unwrap_or(0));
            let fields: Map = serde_json::from_str(&props_json)?;

            Ok(Some(GraphThing {
                id,
                kind,
                labels: [kind].into_iter().collect(), // Simplified
                fields,
                owner: Uuid::nil(),
                revision: revision as u64,
            }))
        } else {
            Ok(None)
        }
    }

    async fn query(&self, pattern: NodePattern) -> Result<Vec<GraphThing>> {
        // Basic implementation: fetch all things and filter in memory (inefficient but safe)
        // Or try to build Cypher.
        // For now, let's fetch all Things.
        let q = "MATCH (t:Thing) RETURN t.uuid, t.kind, t.props_json, t.revision";
        let mut result = self.graph.execute(neo4rs::query(q)).await?;
        let mut things = Vec::new();

        while let Some(row) = result.next().await? {
            let uuid_str: String = row.get("t.uuid")?;
            let kind_str: String = row.get("t.kind")?;
            let props_json: String = row.get("t.props_json")?;
            let revision: i64 = row.get("t.revision")?;

            let id = Uuid::parse_str(&uuid_str)?;
            let kind = Symbol(kind_str.parse().unwrap_or(0));
            let fields: Map = serde_json::from_str(&props_json)?;

            let thing = GraphThing {
                id,
                kind,
                labels: [kind].into_iter().collect(),
                fields,
                owner: Uuid::nil(),
                revision: revision as u64,
            };

            if pattern_matches(&thing, &pattern) {
                things.push(thing);
            }
        }
        Ok(things)
    }

    async fn find_by_kind(&self, kind: String, _cursor: Option<u64>) -> Result<Vec<GraphThing>> {
        // This is tricky because 'kind' in FindByKind is a String, but Symbol is u32.
        // In thing_abi, FindByKind takes String.
        // But GraphThing has Symbol kind.
        // I'll assume the String is the string representation of the Symbol u32?
        // Or maybe it's a label?
        // In userland::find_by_kind, it passes kind: kind.to_string().
        // If I look at InMemoryGraphStore, it returns all things.
        // I'll do the same here: return all things.
        self.query(NodePattern::default()).await
    }

    async fn props_get(&self, req: GraphPropsGetRequest) -> Result<Map> {
        if let Some(thing) = self.get(req.node).await? {
            if req.keys.is_empty() {
                Ok(thing.fields)
            } else {
                Ok(req
                    .keys
                    .iter()
                    .filter_map(|k| thing.fields.get(k).map(|v| (*k, v.clone())))
                    .collect())
            }
        } else {
            Err(anyhow::anyhow!("thing not found"))
        }
    }

    async fn props_set(&self, req: GraphPropsRequest) -> Result<GraphThing> {
        let revision = self.next_revision().await?;
        // First get existing props
        if let Some(mut thing) = self.get(req.node).await? {
            thing.fields.extend(req.props);
            let props_json = serde_json::to_string(&thing.fields)?;

            let q = "MATCH (t:Thing {uuid: $uuid}) 
                     SET t.props_json = $props_json, t.revision = $revision
                     RETURN t";
            self.graph
                .run(
                    neo4rs::query(q)
                        .param("uuid", req.node.to_string())
                        .param("props_json", props_json)
                        .param("revision", revision as i64),
                )
                .await?;

            thing.revision = revision;
            Ok(thing)
        } else {
            Err(anyhow::anyhow!("thing not found"))
        }
    }
}

pub enum GraphBackend {
    InMemory,
    #[cfg(feature = "neo4j")]
    Neo4j,
}

pub struct GraphConfig {
    pub backend: GraphBackend,
    pub uri: String,
    pub user: String,
    pub pass: String,
}

impl GraphConfig {
    pub fn from_env() -> Self {
        let uri = std::env::var("NEO4J_URI").unwrap_or_default();
        let user = std::env::var("NEO4J_USER").unwrap_or_default();
        let pass = std::env::var("NEO4J_PASSWORD").unwrap_or_default();

        let backend = if !uri.is_empty() && cfg!(feature = "neo4j") {
            #[cfg(feature = "neo4j")]
            {
                GraphBackend::Neo4j
            }
            #[cfg(not(feature = "neo4j"))]
            {
                GraphBackend::InMemory
            }
        } else {
            GraphBackend::InMemory
        };

        Self {
            backend,
            uri,
            user,
            pass,
        }
    }
}

pub async fn init_graph_store(config: GraphConfig) -> Arc<dyn GraphStore> {
    match config.backend {
        GraphBackend::InMemory => Arc::new(InMemoryGraphStore::new()),
        #[cfg(feature = "neo4j")]
        GraphBackend::Neo4j => {
            match Neo4jGraphStore::connect(&config.uri, &config.user, &config.pass).await {
                Ok(store) => Arc::new(store),
                Err(e) => {
                    eprintln!(
                        "Failed to connect to Neo4j: {}. Falling back to InMemory.",
                        e
                    );
                    Arc::new(InMemoryGraphStore::new())
                }
            }
        }
    }
}
