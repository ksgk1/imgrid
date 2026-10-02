use std::path::PathBuf;
use std::process::exit;

use image::{ImageReader, RgbaImage};
use rayon::iter::{IndexedParallelIterator, ParallelIterator as _};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.contains(&"--help".to_owned()) {
        println!("Usage: {} [files]", std::env::current_exe().unwrap().file_name().unwrap().to_str().unwrap());
        println!("Will turn all the given images into a square grid, with name: \"grid.png\"");
        exit(0);
    }
    let path_strings = &args[1..];
    let valid_paths: Vec<_> = path_strings.iter().map(PathBuf::from).filter(|p| p.exists()).collect();
    if valid_paths.len() == 1 {
        println!("Only one valid image was given. Exiting...");
        exit(1);
    }

    let grid_elements = valid_paths.len();
    let grid_cols = f32::sqrt(grid_elements as f32).ceil() as u32;
    let grid_rows = (grid_elements as u32).div_ceil(grid_cols);

    println!("Grid size: {grid_rows} x {grid_cols}");

    let images: Vec<RgbaImage> = valid_paths
        .iter()
        .filter_map(|path| {
            let img = ImageReader::open(path).map_err(|e| eprintln!("skipping {}: {e}", path.display())).ok()?.decode().map_err(|e| eprintln!("skipping {}: {e}", path.display())).ok()?.to_rgba8();
            Some(img)
        })
        .collect();

    let dimensions: Vec<(u32, u32)> = images
        .iter()
        .map(|image| {
            let (w, h) = image.dimensions();
            (w, h)
        })
        .collect();

    let (max_image_width, max_image_height) = dimensions.iter().copied().reduce(|(w1, h1), (w2, h2)| (w1.max(w2), h1.max(h2))).expect("no valid images found");
    let (resulting_grid_width, resulting_grid_height) = (max_image_width * grid_cols, max_image_height * grid_rows);
    let mut canvas = RgbaImage::new(resulting_grid_width, resulting_grid_height);

    let grid_image_count = images.len();
    let last_filled_row = (grid_image_count - 1) / grid_rows as usize;
    let last_row_items = grid_image_count - last_filled_row * grid_cols as usize;
    let last_row_shift_px = (grid_cols as usize - last_row_items) * max_image_width as usize / 2;

    canvas.par_pixels_mut().enumerate().for_each(|(pixel_index, pixel_value)| {
        let x = pixel_index % resulting_grid_width as usize;
        let y = pixel_index / resulting_grid_width as usize;
        let slot_row = y / max_image_height as usize;
        let y_in_slot = y % max_image_height as usize;

        let (slot_col, local_x) = if slot_row == last_filled_row && last_row_shift_px > 0 {
            // left padding region = transparent
            if x < last_row_shift_px {
                return;
            }
            let x_shifted = x - last_row_shift_px;
            let shifted_slot_col = x_shifted / max_image_width as usize;
            // right padding region = transparent
            if shifted_slot_col >= last_row_items {
                return;
            }
            (shifted_slot_col, x_shifted % max_image_width as usize)
        } else {
            (x / max_image_width as usize, x % max_image_width as usize)
        };

        let slot_index = slot_row * grid_cols as usize + slot_col;
        let Some(img) = images.get(slot_index) else { return };

        if local_x < img.width() as usize && y_in_slot < img.height() as usize {
            *pixel_value = *img.get_pixel(local_x as u32, y_in_slot as u32);
        }
    });

    canvas.save("grid.png")?;
    Ok(())
}
