import rasterio
import rasterio.warp
from rasterio.crs import CRS
from rasterio.enums import Resampling
from pathlib import Path
import h3.api.numpy_int as h3 
import pandas as pd
import numpy as np
from collections import Counter
import pyproj
from rasterio.warp import calculate_default_transform, reproject, Resampling
import osmnx as ox
from pois_to_h3 import all_shapley_geo_to_h3
import os.path



def count_occurances_break_tie(l):
    # given a list l, return the most frequent item in the list
    # in the event of a tie, choose randomly
    c=Counter(l)
    # get frequency of top 2 items
    freq = c.most_common(2)

    # base case for a single item
    if len(freq)==1:
        return freq[0][0]
    
    # check for ties
    else:
        # they are not equal, return max
        if freq[0][1] != freq[1][1]:
            return freq[0][0]
        else:
            # there is a tie, return random
            return np.random.choice([freq[0][0],freq[1][0]])
        
        

def reproj_bounding_box(bbox, dst_crs, src_crs='EPSG:4326'):
    # reproject a bounding box from src_crs to dst_crs
    
    # initialize transformer
    transformer = pyproj.Transformer.from_crs(src_crs, dst_crs)

    # Reproject the two points defining the bbox to the destination CRS
    # bottom left point 
    xmin, ymin = transformer.transform(bbox[1], bbox[0])
    # top right point
    xmax, ymax = transformer.transform(bbox[3], bbox[2])

    return xmin, ymin, xmax, ymax



def crop_tif_image(tif_path, out_file, bbox, src_crs, dst_crs):
    '''
    tif_path: path to tif file
    out_file: path to save cropped tif file
    bbox: bounding box of the area to crop (should be same crs as src_crs)
    src_crs: crs of the bounding box
    dst_crs: crs of the tif file

    function crops tif file with the bounding box and saves it to out_file
    '''
    # Open the raster file in read mode
    with rasterio.open(tif_path) as src:

        # Reproject the bounding box to the same CRS as the raster
        xmin, ymin, xmax, ymax = reproj_bounding_box(bbox, dst_crs, src_crs)
        # Create a window from the bounding box
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
        # Write the cropped raster to disk
        with rasterio.open(out_file, 'w', **profile) as dst:
            dst.write(subset, 1)



def reproject_tif(cropped_tif_file, dst_crs, reprojected_file):
    '''
    cropped_tif_file: path to cropped tif file
    dst_crs: crs to reproject to
    reprojected_file: path to save reprojected tif file
    returns the transformation from the cropped tif file to the reprojected tif file
    
    function reprojects tif file to dst_crs and saves it to reprojected_file
    '''

    # Reproject the dataset
    with rasterio.open(cropped_tif_file) as src:
        # initialize the transormation parameters
        transform, width, height = calculate_default_transform(
            src.crs, dst_crs, src.width, src.height, *src.bounds)
        kwargs = src.meta.copy()
        kwargs.update({
            'crs': dst_crs,
            'transform': transform,
            'width': width,
            'height': height
        })
        
        # Reproject the data and save it to a new file
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



def tif_to_h3(reprojected_file, transformation, h3_csv, H3_RES):
    '''
    reprojected_file: path to reprojected tif file
    transformation: transformation from reprojected tif file to latlon
    h3_csv: path to save csv file with h3 indices and ghsl codes
    H3_RES: resolution of h3 index
    
    function converts tif file to h3 with ghsl codes as values and saves it to a csv
    '''

    # read in reprojected file
    with rasterio.open(reprojected_file) as dst_band:
        b = dst_band.read(1)

    # get latlon coordinates from reprojected, cropped tif file
    height = b.shape[-2]
    width = b.shape[-1]
    ys, xs = np.meshgrid(np.arange(width), np.arange(height))
    xs, ys = rasterio.transform.xy(transformation, xs, ys)
    xs= np.array(xs).flatten()
    ys = np.array(ys).flatten()


    # https://ghsl.jrc.ec.europa.eu/ghs_buC2022.php for value descriptions
    rel_vals = [11,12,13,14,15,21,22,23,24,25]
    
    # initialize dictionary to store h3 indices and ghsl codes
    # key: h3 index, value: list of ghsl codes
    urban_map = dict()
    # loop through each pixel and add h3 to dictionary
    for x, y, val in zip(ys, xs, b.flatten()):
        if val in rel_vals:
            # if exists as key, append to list
            try:
                urban_map[h3.geo_to_h3(x,y, resolution=H3_RES)].append(val)
            # if doesn't exist as key, create new list
            except:
                urban_map[h3.geo_to_h3(x,y, resolution=H3_RES)]=[val]

    # break ties for most common codes for an h3 index
    # turn into dataframe with columns specified for csv export
    df = pd.DataFrame([(key, count_occurances_break_tie(val)) for key, val in urban_map.items()],
                columns=['h3_index','ghsl_code'])
    
    # add column which specifies residential or not
    # 11,12,13,14,15 are residential, create binary mapping
    val_map = {11:1,12:1,13:1,14:1,15:1,21:0,22:0,23:0,24:0,25:0}
    df['residential_bool'] = df['ghsl_code'].map(val_map)

    # only keep residential areas - we do this in routing right now, bad
    #df = df[df['residential_bool']==1]

    df.to_csv(h3_csv, index=False)


'''
GHSL label meanings:
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


def city_boundaries_to_h3(city_names):
    '''
    city_names: list of city names

    output: list of h3 indices for city boundaries and the bounding box of the city
            bbox pois is a slightly larger bounding box for getting pois
    '''
    # get city bounding box
    city_geo = ox.geocode_to_gdf(city_names)  #Look into getting this directly from file

    # get bounding box of area
    minx, miny, maxx, maxy = city_geo.total_bounds
    bbox = [minx, miny, maxx, maxy]
    # increase bounding box by 0.03 degrees in all directions (around 3 km) - currently unused
    bbox_pois = [minx-0.03, miny-0.03, maxx+0.03, maxy+0.03]

    city_poly = city_geo.geometry.values
    # convert to h3
    city_bounds_h3 = [all_shapley_geo_to_h3(p, 12) for p in city_poly]
    # flatten h3 list
    city_bounds_h3 = [item for sublist in city_bounds_h3 for item in sublist]

    return city_bounds_h3, bbox, bbox_pois




def get_origins(H3_RES, city_names, bbox, tif_path, city_bounds_h3):
    '''
    H3_RES: resolution of h3 index
    city_names: list of city names
    bbox: bounding box of the area to crop
    tif_path: path to tif file
    city_bounds_h3: list of h3 indices for city boundaries
    
    output: dataframe with h3 indices as origins within the city boundaries
    '''

    # filenames
    file_name = "_".join(city_names)
    cropped_tif_file = f'../resources/origins/{file_name}_subset.tif'
    reprojected_file = f'../resources/origins/{file_name}_reprojected.tif'
    h3_csv = f'../resources/origins/{file_name}_ghsl_h3_codes.csv'
    final_origins = f'../resources/origins/{file_name}_origins_clean.csv'

    # check if files already exist
    if os.path.exists(final_origins):
        print(f"file already exists for {file_name}")
        return pd.read_csv(final_origins)

    # CROP
    # cropping crs from latlon --> tif crs
    src_crs = 'EPSG:4326'
    dst_crs = 'ESRI:54009'
    crop_tif_image(tif_path, cropped_tif_file, bbox, src_crs, dst_crs)

    # Reproj
    # reprojection goes from tif crs --> latlon 
    src_crs = 'ESRI:54009'
    dst_crs = 'EPSG:4326'
    transformation = reproject_tif(cropped_tif_file, dst_crs, reprojected_file)

    # TIF TO H3
    H3_RES = 12
    tif_to_h3(reprojected_file, transformation, h3_csv, H3_RES)

    # subset the ghsl h3 codes to only include those that are within the city bounds
    origins = pd.read_csv(h3_csv)
    overlap = set(origins['h3_index']).intersection(set(city_bounds_h3))
    # only keep dataframe rows that have h3 index in overlap
    origins = origins[origins['h3_index'].isin(overlap)]
    origins.to_csv(final_origins, index=False)

    # return dataframe with h3 indices as origins within the city boundaries
    return origins