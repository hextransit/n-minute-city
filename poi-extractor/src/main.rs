use gdal::Dataset;


fn main() -> anyhow::Result<()> {
    let ds = Dataset::open("resources/GHS_BUILT_C_FUN_E2018_GLOBE_R2022A_54009_10_V1_0_R3_C19.tif")?;
    println!("This {} is in '{}' and has {} bands.", ds.driver().long_name(), ds.spatial_ref()?.name()?, ds.raster_count());
    let band = ds.rasterband(1)?;
    let transform = ds.geo_transform()?;
    let min_max = band.compute_raster_min_max(true)?;
    let (min, max) = (min_max.min, min_max.max);
    println!("min: {}, max: {}", min, max);
    println!("{:?}", band.overview_count()?);
    
    let size = (band.x_size(), band.y_size());
    let values = band.read_as::<u8>((0, 0), size, size, None)?;
    println!("{:?}", values.data);
    Ok(())
}
