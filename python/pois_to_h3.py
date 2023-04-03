from pyrosm import OSM
from pyrosm import get_data
import pandas as pd
import h3.api.numpy_int as h3 
import shapely
import matplotlib.pyplot as plt
import geopandas as gpd
from shapely.geometry import MultiPolygon
from shapely.geometry import Polygon
import contextily as cx
import os.path


def flatten(lst):
    return [item for sublist in lst for item in (flatten(sublist) if isinstance(sublist, list) else [sublist])]

def swap_xy(geom):
    if geom.is_empty:
        return geom

    if geom.has_z:
        def swap_xy_coords(coords):
            for x, y, z in coords:
                yield (y, x, z)
    else:
        def swap_xy_coords(coords):
            for x, y in coords:
                yield (y, x)

    # Process coordinates from each supported geometry type
    if geom.type in ('Point', 'LineString', 'LinearRing'):
        return type(geom)(list(swap_xy_coords(geom.coords)))
    elif geom.type == 'Polygon':
        ring = geom.exterior
        shell = type(ring)(list(swap_xy_coords(ring.coords)))
        holes = list(geom.interiors)
        for pos, ring in enumerate(holes):
            holes[pos] = type(ring)(list(swap_xy_coords(ring.coords)))
        return type(geom)(shell, holes)
    elif geom.type.startswith('Multi') or geom.type == 'GeometryCollection':
        # Recursive call
        return type(geom)([swap_xy(part) for part in geom.geoms])
    else:
        raise ValueError('Type %r not recognized' % geom.type)

def h3_list_to_multi_poly(h3_list):
    h3_polygon = h3.h3_set_to_multi_polygon(h3_list)
    # for some reason you can't go straight to multiploly ////:
    return MultiPolygon([Polygon(p[0]) for p in h3_polygon])

def LineString_to_hex(line, H3_RES):
    l_coords = [x for x in line.coords]
    start = h3.geo_to_h3(l_coords[0][0], l_coords[0][1], H3_RES)
    end = h3.geo_to_h3(l_coords[-1][0], l_coords[-1][1], H3_RES)
    return h3.h3_line(start,end)

def all_shapley_geo_to_h3(obj, H3_RES):
    geom_type = obj.geom_type
    # assert geom_type valid at some point

    # shapely and h3 swap x and y:
    obj = swap_xy(obj)

    if geom_type=='MultiPolygon':
        # this will break in a different version of shapley, use this instead of iterating polys in multipoly: 
        # multi_poly.coords or .geoms
        return [ind for p in obj.geoms for ind in h3.polyfill(shapely.geometry.mapping(p), H3_RES)] # loop through polys and flatten
    elif geom_type=='Polygon':
        return h3.polyfill(shapely.geometry.mapping(obj), H3_RES)
    elif geom_type=='MultiLineString':
        # this will break in a different version of shapley, use this instead of iterating lines in multi_line: 
        # obj.coords or obj.geoms
        return [ind for l in obj.geoms for ind in LineString_to_hex(l,H3_RES)]
    elif geom_type=='LineString':
        return LineString_to_hex(obj, H3_RES)
    elif geom_type=='Point':
        return h3.geo_to_h3(obj.x, obj.y, H3_RES)
    else:
        print(f"unimplemented geom type: {geom_type}")
    

def plot_h3_and_geo(h3_index_list, shapely_geo):
    p = gpd.GeoSeries(shapely_geo)
    p2 = gpd.GeoSeries(h3_list_to_multi_poly(h3_index_list))
    gdf1 = gpd.GeoDataFrame(geometry=p)
    gdf2 = gpd.GeoDataFrame(geometry=p2)

    # Create a figure and axis
    fig, ax = plt.subplots()

    # Plot the GeoDataFrames on the axis
    gdf1.plot(ax=ax, color='blue', alpha=0.5)
    gdf2.plot(ax=ax, color='red', alpha=0.5)

    plt.show()


def osm_to_manual_category(tag, osm_tag_mapping):
    # faster than searching keys to try, except
    try:
        return osm_tag_mapping[tag]
    except:
        return tag

# some tags don't have headers, find manually in tags
def tag_conditions(tags, healthcare_list):
    # search for healthcare tag substring, return match
    s = [s for s in healthcare_list if s in tags]
    if s:
        return s[0]
    elif "sport" in tags:
        return "sport" 
    else:
        return None

def df_manipulations(pois, H3_RES, osm_filter, category_set, osm_tag_mapping):
    '''
    input: raw poi df direct from pyrosm
    output: df with columns h3_index and category
    '''

    pois["poi_type"] = pois["amenity"]
    pois["poi_type"] = pois["poi_type"].fillna(pois["shop"])
    pois["poi_type"] = pois["poi_type"].fillna(pois["leisure"])

    # some pois don't have a poi_type, find them in tags
    pois['tags'] = pois['tags'].astype(str)
    pois['no_header'] = pois.apply(lambda x: tag_conditions(x.tags, osm_filter['healthcare']), axis=1)
    pois["poi_type"] = pois["poi_type"].fillna(pois["no_header"])

    # convert poi_type to n minute city category
    pois['category'] = pois["poi_type"].apply(lambda x: osm_to_manual_category(x, osm_tag_mapping))

    # convert all geometry to h3
    h3_df = pois[['category','poi_type','geometry']].copy()
    h3_df['h3_index'] = pois.apply(lambda x: all_shapley_geo_to_h3(x.geometry, H3_RES), axis=1)
    # make one h3 index on each row
    h3_df = h3_df.explode('h3_index')
    # for some reason other categories are still in the df - not many - put earlier for efficiency
    h3_df = h3_df[h3_df['category'].isin(category_set)]
    # get rid of nans
    h3_df = h3_df[~h3_df['h3_index'].isna()]
    
    return h3_df[['h3_index','category']]

def get_pois_h3(pbf_path, osm_filter, H3_RES, category_set, osm_tag_mapping, municipality):

    file_name = "_".join(municipality)
    final_destinations = f'../resources/destinations/{file_name}_destinations_clean.csv'
    if os.path.exists(final_destinations):
        print(f"file already exists for {file_name}")
        return pd.read_csv(final_destinations)
    osm = OSM(pbf_path)
    pois = osm.get_pois(custom_filter=osm_filter)
    df = df_manipulations(pois, H3_RES, osm_filter, category_set, osm_tag_mapping)
    df.to_csv(final_destinations, index=False)

    return df