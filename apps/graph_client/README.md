# Graph Client

This is a test client for the ThingOS Host Graph ABI.
It connects to the `HostRuntime` and performs basic graph operations (fiat, that, find).

## Running

To run the client with the in-memory graph store:

```bash
cargo run -p graph_client --target x86_64-unknown-linux-gnu
```

## Using Neo4j

To use a real Neo4j database:

1. Start Neo4j using Docker Compose:
   ```bash
   docker compose -f ../../docker-compose.neo4j.yml up -d
   ```

2. Run the client with environment variables:
   ```bash
   NEO4J_URI=bolt://localhost:7687 NEO4J_USER=neo4j NEO4J_PASS=password cargo run -p graph_client --target x86_64-unknown-linux-gnu
   ```

The `HostRuntime` will automatically connect to Neo4j if `NEO4J_URI` is set.
