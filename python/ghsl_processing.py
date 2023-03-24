import rasterio
import matplotlib.pyplot as plt
from matplotlib import colors
from rasterio.plot import show
import rasterio.warp
from rasterio.crs import CRS
from rasterio.enums import Resampling
import sys
from pathlib import Path
import json
import h3.api.numpy_int as h3 
import pandas as pd
import numpy as np
import geopandas as gpd
import shapely as shp
from shapely.geometry import mapping, shape
from collections import Counter
import pyproj
from rasterio.warp import calculate_default_transform, reproject, Resampling
from shapely.geometry import Point
import contextily as cx
import osmnx as ox
from pois_to_h3 import all_shapley_geo_to_h3



def count_occurances_break_tie(l):
    # given a list, return the most frequent item in the list
    # in the event of a tie, choose randomly
    c=Counter(l)
    # get frequency of top 2 items
    freq = c.most_common(2)

    if len(freq)==1:
        return freq[0][0]
    
    # check for ties
    else:
        if freq[0][1] != freq[1][1]:
            return freq[0][0]
        else:
            #print("tie for most common!")
            return np.random.choice([freq[0][0],freq[1][0]])
        
        
# reproject bounding box
# Define the point coordinates in the source CRS
# paste csv bounding box from this website https://boundingbox.klokantech.com/
def reproj_bounding_box(bbox, dst_crs, src_crs='EPSG:4326'):
    # Define the source and destination coordinate reference systems
    transformer = pyproj.Transformer.from_crs(src_crs, dst_crs)

    # Reproject the point to the destination CRS
    xmin, ymin = transformer.transform(bbox[1], bbox[0])
    xmax, ymax = transformer.transform(bbox[3], bbox[2])

    return xmin, ymin, xmax, ymax


def crop_tif_image(tif_path, out_file, bbox, src_crs, dst_crs):
    with rasterio.open(tif_path) as src:

        # Get the raster size
        rows, cols = src.shape

        xmin, ymin, xmax, ymax = reproj_bounding_box(bbox, dst_crs, src_crs)
        window = rasterio.windows.from_bounds(xmin, ymin, xmax, ymax, src.transform)

        # Read the subset region from the raster
        subset = src.read(1, window=window)

        # Create a new raster file for the subset
        profile = src.profile
        profile.update({
            'width': window.width,
            'height': window.height,
            'transform': src.window_transform(window)
        })
        with rasterio.open(out_file, 'w', **profile) as dst:

            # Write the subset to the new raster file
            dst.write(subset, 1)

# reproject tif file to destination crs
def reproject_tif(cropped_tif_file, dst_crs, reprojected_file):
    with rasterio.open(cropped_tif_file) as src:
        transform, width, height = calculate_default_transform(
            src.crs, dst_crs, src.width, src.height, *src.bounds)
        kwargs = src.meta.copy()
        kwargs.update({
            'crs': dst_crs,
            'transform': transform,
            'width': width,
            'height': height
        })
        
        with rasterio.open(reprojected_file, 'w', **kwargs) as dst:
            b, trans = reproject(
                source=rasterio.band(src, 1),
                destination=rasterio.band(dst, 1),
                src_transform=src.transform,
                src_crs=src.crs,
                dst_transform=transform,
                dst_crs=dst_crs,
                resampling=Resampling.nearest)
            
    return trans


# convert tif with ghsl codes to h3 with ghsl codes as values
def tif_to_h3(reprojected_file, transformation, h3_csv, H3_RES):

    urban_map = dict()

    # read in reprojected file
    with rasterio.open(reprojected_file) as dst_band:
        b = dst_band.read(1)

    height = b.shape[-2]
    width = b.shape[-1]
    ys, xs = np.meshgrid(np.arange(width), np.arange(height))
    xs, ys = rasterio.transform.xy(transformation, xs, ys)
    xs= np.array(xs).flatten()
    ys = np.array(ys).flatten()

    rel_vals = [11,12,13,14,15,21,22,23,24,25]

    # https://ghsl.jrc.ec.europa.eu/ghs_buC2022.php for value descriptions
    for x,y, val in zip(ys, xs, b.flatten()):
        if val in rel_vals:
            try:
                urban_map[h3.geo_to_h3(x,y, resolution=H3_RES)].append(val)
            except:
                urban_map[h3.geo_to_h3(x,y, resolution=H3_RES)]=[val]

    # break ties for most common codes for an h3 index
    # turn into dataframe with columns specified for csv export
    df = pd.DataFrame([(key, count_occurances_break_tie(val)) for key, val in urban_map.items()],
                columns=['h3_index','ghsl_code'])
    
    # add column which specifies residential or not
    val_map = {11:1,12:1,13:1,14:1,15:1,21:0,22:0,23:0,24:0,25:0}
    df['residential_bool'] = df['ghsl_code'].map(val_map)

    df.to_csv(h3_csv, index=False)


def city_boundaries_to_h3(city_names):
    '''
    city_names: list of city names

    output: list of h3 indices for city boundaries and the bounding box of the city
            bbox pois is a slightly larger bounding box for getting pois
    '''
    # get city bounding box
    city_geo = ox.geocode_to_gdf(city_names)  #Look into getting this directly from file
    # make bounding box bigger
    minx, miny, maxx, maxy = city_geo.total_bounds
    bbox = [minx, miny, maxx, maxy]

    # increase bounding box by 0.03 degrees in all directions (around 3 km)
    bbox_pois = [minx-0.03, miny-0.03, maxx+0.03, maxy+0.03]

    city_poly = city_geo.geometry.values
    # convert to h3
    city_bounds_h3 = [all_shapley_geo_to_h3(p, 12) for p in city_poly]
    # flatten h3 list
    city_bounds_h3 = [item for sublist in city_bounds_h3 for item in sublist]

    return city_bounds_h3, bbox, bbox_pois

'''
label meanings:
01 : MSZ, open spaces, low vegetation surfaces NDVI <= 0.3
02 : MSZ, open spaces, medium vegetation surfaces 0.3 < NDVI <=0.5
03 : MSZ, open spaces, high vegetation surfaces NDVI > 0.5
04 : MSZ, open spaces, water surfaces LAND < 0.5
05 : MSZ, open spaces, road surfaces
11 : MSZ, built spaces, residential, building height <= 3m
12 : MSZ, built spaces, residential, 3m < building height <= 6m
13 : MSZ, built spaces, residential, 6m < building height <= 15m
14 : MSZ, built spaces, residential, 15m < building height <= 30m
15 : MSZ, built spaces, residential, building height > 30m
21 : MSZ, built spaces, non-residential, building height <= 3m
22 : MSZ, built spaces, non-residential, 3m < building height <= 6m
23 : MSZ, built spaces, non-residential, 6m < building height <= 15m
24 : MSZ, built spaces, non-residential, 15m < building height <= 30m
25 : MSZ, built spaces, non-residential, building height > 30m
''';