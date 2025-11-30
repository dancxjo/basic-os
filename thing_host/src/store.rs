use crate::symbols;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex};
use thing_abi::{
    GraphEdge, GraphFiatRequest, GraphGetRequest, GraphLinkRequest, GraphPropsGetRequest,
    GraphPropsRequest, GraphThing, Map, NodePattern, Symbol,
};
use uuid::Uuid;

#[cfg(feature = "neo4j")]
use neo4rs::query;

const FIND_PAGE_SIZE: usize = 64;

#[async_trait]
pub trait GraphStore: Send + Sync {
    async fn fiat(&self, req: GraphFiatRequest) -> Result<GraphThing>;
    async fn link(&self, req: GraphLinkRequest) -> Result<GraphEdge>;
    async fn get(&self, req: GraphGetRequest) -> Result<Vec<GraphThing>>;
    async fn find_by_kind(
        &self,
        kind: String,
        cursor: Option<u64>,
    ) -> Result<(Vec<GraphThing>, Option<u64>)>;
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
        self.revision.fetch_add(1, AtomicOrdering::SeqCst)
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
        let labels_set = labels.into_iter().collect();
        let thing = GraphThing {
            id,
            kind: req.kind,
            labels: labels_set,
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

    async fn get(&self, req: GraphGetRequest) -> Result<Vec<GraphThing>> {
        match req {
            GraphGetRequest::Thing(id) => Ok(self
                .things
                .lock()
                .unwrap()
                .get(&id)
                .cloned()
                .into_iter()
                .collect()),
            GraphGetRequest::Pattern(pattern) => {
                let things = self.things.lock().unwrap();
                let result = things
                    .values()
                    .filter(|thing| pattern_matches(thing, &pattern))
                    .cloned()
                    .collect();
                Ok(result)
            }
        }
    }

    async fn find_by_kind(
        &self,
        kind: String,
        cursor: Option<u64>,
    ) -> Result<(Vec<GraphThing>, Option<u64>)> {
        let Some(symbol) = symbols::symbol_from_str(&kind) else {
            return Ok((Vec::new(), None));
        };
        let things = self.things.lock().unwrap();
        let mut matches: Vec<_> = things
            .values()
            .filter(|thing| thing.kind == symbol)
            .cloned()
            .collect();
        matches.sort_by_key(|thing| thing.id);

        let skip = cursor.unwrap_or(0) as usize;
        if skip >= matches.len() {
            return Ok((Vec::new(), None));
        }

        let mut slice: Vec<_> = matches
            .into_iter()
            .skip(skip)
            .take(FIND_PAGE_SIZE + 1)
            .collect();
        let next_cursor = if slice.len() > FIND_PAGE_SIZE {
            slice.pop();
            Some((skip + FIND_PAGE_SIZE) as u64)
        } else {
            None
        };
        Ok((slice, next_cursor))
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
            Err(anyhow!("thing not found"))
        }
    }

    async fn props_set(&self, req: GraphPropsRequest) -> Result<GraphThing> {
        let mut things = self.things.lock().unwrap();
        if let Some(thing) = things.get_mut(&req.node) {
            thing.fields.extend(req.props);
            thing.revision = self.next_revision();
            Ok(thing.clone())
        } else {
            Err(anyhow!("thing not found"))
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
    pub async fn connect(config: &Neo4jConfig) -> Result<Self> {
        let neo_config = neo4rs::ConfigBuilder::new()
            .uri(&config.uri)
            .user(&config.user)
            .password(&config.password)
            .build()?;
        let graph = Arc::new(neo4rs::Graph::connect(neo_config).await?);
        Ok(Self { graph })
    }

    async fn next_revision(&self) -> Result<u64> {
        let mut result = self
            .graph
            .execute(query(
                "MERGE (c:Counter {name: 'revision'})
                 ON CREATE SET c.value = 1
                 ON MATCH SET c.value = c.value + 1
                 RETURN c.value as value",
            ))
            .await?;
        if let Some(row) = result.next().await? {
            Ok(row.get::<i64>("value")? as u64)
        } else {
            Ok(1)
        }
    }

    fn encode_labels(labels: &BTreeSet<Symbol>) -> Vec<i64> {
        labels.iter().map(|sym| sym.raw() as i64).collect()
    }

    fn decode_labels(raw: Vec<i64>) -> BTreeSet<Symbol> {
        raw.into_iter().map(|value| Symbol(value as u32)).collect()
    }

    fn symbol_display(sym: Symbol) -> String {
        symbols::symbol_name(sym)
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("sym_{:06x}", sym.raw()))
    }

    async fn load_things(&self, cypher: neo4rs::Query) -> Result<Vec<GraphThing>> {
        let mut result = self.graph.execute(cypher).await?;
        let mut things = Vec::new();
        while let Some(row) = result.next().await? {
            things.push(Self::row_to_thing(&row)?);
        }
        Ok(things)
    }

    fn row_to_thing(row: &neo4rs::Row) -> Result<GraphThing> {
        let uuid_str: String = row.get("uuid")?;
        let kind_raw: i64 = row.get("kind")?;
        let labels_raw: Vec<i64> = row.get("labels")?;
        let props_json: String = row.get("props_json")?;
        let revision: i64 = row.get("revision")?;
        let owner_str: Option<String> = row.get("owner").ok();

        let id = Uuid::parse_str(&uuid_str).map_err(|e| anyhow!(e.to_string()))?;
        let fields: Map = serde_json::from_str(&props_json)?;
        let owner = owner_str
            .and_then(|s| Uuid::parse_str(&s).ok())
            .unwrap_or_else(Uuid::nil);

        Ok(GraphThing {
            id,
            kind: Symbol(kind_raw as u32),
            labels: Self::decode_labels(labels_raw),
            fields,
            owner,
            revision: revision as u64,
        })
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
        let label_set: BTreeSet<_> = labels.iter().copied().collect();
        let labels_vec = Self::encode_labels(&label_set);
        let props_json = serde_json::to_string(&req.fields)?;

        let kind_name = Self::symbol_display(req.kind);
        let owner = Uuid::nil().to_string();

        self.graph
            .run(
                query(
                    "MERGE (t:Thing {uuid: $uuid})
                     SET t.kind = $kind,
                         t.kind_name = $kind_name,
                         t.labels = $labels,
                         t.props_json = $props,
                         t.owner = $owner,
                         t.revision = $revision",
                )
                .param("uuid", id.to_string())
                .param("kind", req.kind.raw() as i64)
                .param("kind_name", kind_name)
                .param("labels", labels_vec)
                .param("props", props_json)
                .param("owner", owner)
                .param("revision", revision as i64),
            )
            .await?;

        Ok(GraphThing {
            id,
            kind: req.kind,
            labels: label_set,
            fields: req.fields,
            owner: Uuid::nil(),
            revision,
        })
    }

    async fn link(&self, req: GraphLinkRequest) -> Result<GraphEdge> {
        let id = req.id.unwrap_or_else(|| thing_abi::next_uuid());
        let revision = self.next_revision().await?;
        let props_json = serde_json::to_string(&req.props)?;
        let owner = Uuid::nil().to_string();
        let pred_name = Self::symbol_display(req.kind);

        self.graph
            .run(
                query(
                    "MERGE (src:Thing {uuid: $src})
                     ON CREATE SET src.kind = 0,
                                   src.kind_name = 'unknown',
                                   src.labels = [],
                                   src.props_json = '{}',
                                   src.owner = $owner,
                                   src.revision = 0
                     MERGE (dst:Thing {uuid: $dst})
                     ON CREATE SET dst.kind = 0,
                                   dst.kind_name = 'unknown',
                                   dst.labels = [],
                                   dst.props_json = '{}',
                                   dst.owner = $owner,
                                   dst.revision = 0
                     MERGE (src)-[r:LINK {uuid: $uuid}]->(dst)
                     SET r.pred = $pred,
                         r.pred_name = $pred_name,
                         r.props_json = $props,
                         r.revision = $revision",
                )
                .param("src", req.from.to_string())
                .param("dst", req.to.to_string())
                .param("uuid", id.to_string())
                .param("pred", req.kind.raw() as i64)
                .param("pred_name", pred_name)
                .param("props", props_json)
                .param("revision", revision as i64)
                .param("owner", owner),
            )
            .await?;

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

    async fn get(&self, req: GraphGetRequest) -> Result<Vec<GraphThing>> {
        match req {
            GraphGetRequest::Thing(id) => {
                let query = query(
                    "MATCH (t:Thing {uuid: $uuid})
                     RETURN t.uuid AS uuid,
                            t.kind AS kind,
                            t.labels AS labels,
                            t.props_json AS props_json,
                            t.owner AS owner,
                            t.revision AS revision",
                )
                .param("uuid", id.to_string());
                self.load_things(query).await
            }
            GraphGetRequest::Pattern(pattern) => {
                let query = query(
                    "MATCH (t:Thing)
                     RETURN t.uuid AS uuid,
                            t.kind AS kind,
                            t.labels AS labels,
                            t.props_json AS props_json,
                            t.owner AS owner,
                            t.revision AS revision",
                );
                let things = self.load_things(query).await?;
                Ok(things
                    .into_iter()
                    .filter(|thing| pattern_matches(thing, &pattern))
                    .collect())
            }
        }
    }

    async fn find_by_kind(
        &self,
        kind: String,
        cursor: Option<u64>,
    ) -> Result<(Vec<GraphThing>, Option<u64>)> {
        let Some(symbol) = symbols::symbol_from_str(&kind) else {
            return Ok((Vec::new(), None));
        };
        let skip = cursor.unwrap_or(0);
        let limit = (FIND_PAGE_SIZE + 1) as i64;
        let query = query(
            "MATCH (t:Thing)
             WHERE t.kind = $kind
             RETURN t.uuid AS uuid,
                    t.kind AS kind,
                    t.labels AS labels,
                    t.props_json AS props_json,
                    t.owner AS owner,
                    t.revision AS revision
             ORDER BY t.uuid
             SKIP $skip
             LIMIT $limit",
        )
        .param("kind", symbol.raw() as i64)
        .param("skip", skip as i64)
        .param("limit", limit);

        let mut entries = self.load_things(query).await?;
        let next_cursor = if entries.len() > FIND_PAGE_SIZE {
            entries.truncate(FIND_PAGE_SIZE);
            Some(skip + FIND_PAGE_SIZE as u64)
        } else {
            None
        };
        Ok((entries, next_cursor))
    }

    async fn props_get(&self, req: GraphPropsGetRequest) -> Result<Map> {
        let mut things = self.get(GraphGetRequest::Thing(req.node)).await?;
        let Some(thing) = things.pop() else {
            return Err(anyhow!("thing not found"));
        };
        if req.keys.is_empty() {
            Ok(thing.fields)
        } else {
            Ok(req
                .keys
                .iter()
                .filter_map(|k| thing.fields.get(k).map(|v| (*k, v.clone())))
                .collect())
        }
    }

    async fn props_set(&self, req: GraphPropsRequest) -> Result<GraphThing> {
        let mut things = self.get(GraphGetRequest::Thing(req.node)).await?;
        let Some(mut thing) = things.pop() else {
            return Err(anyhow!("thing not found"));
        };
        thing.fields.extend(req.props.clone());
        thing.revision = self.next_revision().await?;
        let props_json = serde_json::to_string(&thing.fields)?;

        self.graph
            .run(
                query(
                    "MATCH (t:Thing {uuid: $uuid})
                     SET t.props_json = $props,
                         t.revision = $revision",
                )
                .param("uuid", req.node.to_string())
                .param("props", props_json)
                .param("revision", thing.revision as i64),
            )
            .await?;

        Ok(thing)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphBackend {
    InMemory,
    #[cfg(feature = "neo4j")]
    Neo4j,
}

pub struct Neo4jConfig {
    pub uri: String,
    pub user: String,
    pub password: String,
}

pub struct GraphConfig {
    pub backend: GraphBackend,
    pub neo4j: Neo4jConfig,
}

impl GraphConfig {
    pub fn from_env() -> Self {
        let requested = std::env::var("GRAPH_BACKEND").unwrap_or_else(|_| "in-memory".into());
        let backend = match requested.trim().to_ascii_lowercase().as_str() {
            "neo4j" => {
                #[cfg(feature = "neo4j")]
                {
                    GraphBackend::Neo4j
                }
                #[cfg(not(feature = "neo4j"))]
                {
                    eprintln!(
                        "GRAPH_BACKEND=neo4j requested, but thing_host was built without the neo4j feature; defaulting to the in-memory backend."
                    );
                    GraphBackend::InMemory
                }
            }
            _ => GraphBackend::InMemory,
        };

        let uri = std::env::var("NEO4J_URI").unwrap_or_else(|_| "bolt://127.0.0.1:7687".into());
        let user = std::env::var("NEO4J_USER").unwrap_or_else(|_| "neo4j".into());
        let password = std::env::var("NEO4J_PASSWORD").unwrap_or_else(|_| "secret".into());

        Self {
            backend,
            neo4j: Neo4jConfig {
                uri,
                user,
                password,
            },
        }
    }
}

pub async fn init_graph_store(config: &GraphConfig) -> (Arc<dyn GraphStore>, GraphBackend) {
    match config.backend {
        GraphBackend::InMemory => (Arc::new(InMemoryGraphStore::new()), GraphBackend::InMemory),
        #[cfg(feature = "neo4j")]
        GraphBackend::Neo4j => match Neo4jGraphStore::connect(&config.neo4j).await {
            Ok(store) => (Arc::new(store), GraphBackend::Neo4j),
            Err(err) => {
                eprintln!(
                    "Failed to connect to Neo4j at {}: {}. Falling back to in-memory.",
                    config.neo4j.uri, err
                );
                (Arc::new(InMemoryGraphStore::new()), GraphBackend::InMemory)
            }
        },
    }
}

impl std::fmt::Display for GraphBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphBackend::InMemory => f.write_str("InMemory"),
            #[cfg(feature = "neo4j")]
            GraphBackend::Neo4j => f.write_str("Neo4j"),
        }
    }
}
