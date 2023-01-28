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
* AStar

## Example
```rust
use graph_ds::{Graph, Node, Edge};
use graph_ds::hexagon_graph::cell::Cell;

let mut graph = Graph::<Cell>::new();
let a = Cell { a: 60, b: -33, radius: 24, layer: 3}
let b = Cell { a: 34, b: -24, radius: 24, layer: 3}
let c = Cell { a: 19, b: 1, radius: 24, layer: 3}

graph.graph.build_and_add_egde(a, b, Some(1.0), None);
graph.graph.build_and_add_egde(b, c, Some(1.0), None);

let (path, distance)= graph.bfs(a, Some(c))?;
```

## TODO
- [ ] add layering support to node and edge creation
- [ ] support edge and node removal
