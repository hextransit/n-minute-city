# The n-minute city

Evaluating the progress of cities towards the 15-minute city using multi-layer networks.

## python
In the python folder, you can find notebooks used for data analysis and POI extraction.

## graph-ds [(Docs)](graph-ds/README.md)
A graph data structure library implemented in rust with a python interface. Built to be memory efficient and allow for parallel traversals. 

To use the python interface, install the wheel using pip:
```bash
# linux
pip install wheels/graph_ds-0.1.0-cp37-cp37m-manylinux2010_x86_64.whl

# mac (intel)
pip install wheels/graph_ds-0.1.0-cp37-cp37m-macosx_10_9_x86_64.whl
```

The wheels are generated automatically by GitHub Actions. 

Graphs can be created from OSM and GTFS data using the `create` function. The graph will be multi-layered, with a base layer of hexagon cells for the walking network, a layer for the bike network and one additional layer for every route in the GTFS data. The edge weights represent time in minutes. The chosen H3 hexagon resolution is 12.

There are no direct connections from the bike layer to the transit layers.

### Using PyH3Graph or PyCellGraph from python

create a new graph object:
```python
graph = PyH3Graph(weight_options={
    bike_penalty: 1.0,
    wait_time_multiplier: 1.0,
    walk_speed: 1.4,
    bike_speed: 4.5,
} | {}>, k_ring=2, layers="all")
graph.create(osm_path="<path>", gtfs_paths=["<path>"])
```
The `layers` keyword argument allows to specify the layers the graph should contain after processing. The walk network is always included. Supported layer tags are: `all` (default), `walk`, `walk+bike`, `walk+transit`.


**PyH3Graph** exposes two functions for pathfinding:
* `matrix_distance` - returns the distance between all hexagon cells
* `dijkstra_path` - returns the path between two hexagon cells

H3 cells need to be input in their u64 integer representation. Only cells on the base layer are valid start and end points.

```python
# get the distance matrix
distances = graph.matrix_distance(origins=[u64], destinations=[u64], hour_of_week=int, infinity=Optional[float], dynamic_infinity=bool)

path = graph.dijkstra_path(start=u64, end=u64, hour_of_week=Optional[int])
```

For testing purposes, you can obtain a random node from the graph by calling `graph.get_random_node()`

The optional `hour_of_week` parameter allows the transit layers to model expected wait time based on the time of day. The input expects an integer representing the hour of the week, starting at 0 for Monday 00:00 and ending at 167 for Sunday 23:00.

The parameters `infinity` and `dynamic_infinity` are used to set the maximum distance between two cells. If `dynamic_infinity` is set to `True`, the pathfinding will lower the ininity value during calculation. This is only useful when searching for minimum distances.

If a given index is not present in the graph, the pathfinding will attempt to map it to an index nearby, with a maximum permitted distance of 2 cells. If no nearby index is found, an empty list will be returned for that origin.

**PyCellGraph** exposes the `matrix_distance` function without the `hour_of_week` parameter. It also expects the input to be lists of u64 cell ids. 
