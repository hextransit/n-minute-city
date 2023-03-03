# graph-ds
Generic, high-performance graph data structure with support for parallel traversals.

## Features
* Hexagonal multi-layer graph
```rust
let graph = Graph<Cell>;
```
* Hexagonal H3-backed graph (+ additional layer support)
```rust
let graph = Graph<H3Cell>;
```
* generic u64 graph
```rust
let graph = Graph<u64>;
```

## Algorithms
* BFS + parallel matrix BFS
* AStar + parallel matrix Astar

## Example
```rust
use graph_ds::{Graph, Node, Edge};
use graph_ds::hexagon_graph::cell::Cell;

let mut graph = Graph::<Cell>::new();
let a = Cell { a: 60, b: -33, radius: 24, layer: 3}
let b = Cell { a: 34, b: -24, radius: 24, layer: 3}
let c = Cell { a: 19, b: 1, radius: 24, layer: 3}

graph.graph.build_and_add_egde(a, b, Some(1.0), None, None);
graph.graph.build_and_add_egde(b, c, Some(1.0), None, None);

let (path, distance) = graph.bfs(start, Some(end), &None)?;
```
## Layering
The graphs support explicit layer information to be stored on the nodes. For `Cell`, the layer is part of the u64 ID, for `H3Cell`, the layer is stored in the `layer` field. 

H3 graphs can be created directly from OSM and GTFS data, for which this library includes parsing functions. The multi-layered graph will be set up as follows:
* base layer (walking network), ID: $-1$
* bike layer, ID: $-2$
* transit layers, ID: `<route_id>` (the route ID is a positive integer $r>=0$)

## TODO
- [ ] support node removal
- [ ] add flow algorithms
- [ ] extract largest component function
