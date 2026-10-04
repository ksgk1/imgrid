use std::path::{Path, PathBuf};
use std::process::exit;

use image::{ImageReader, RgbaImage};
use rayon::iter::{IndexedParallelIterator, ParallelIterator as _};

const fn ceil_sqrt(n: usize) -> usize {
    let r = n.isqrt();
    if r * r == n { r } else { r + 1 }
}

struct Grid {
    cols:              usize,
    rows:              usize,
    slot_w:            usize,
    slot_h:            usize,
    last_filled_row:   usize,
    last_row_items:    usize,
    last_row_shift_px: usize,
}

impl Grid {
    const fn new(image_count: usize, slot_w: usize, slot_h: usize) -> Self {
        let cols = ceil_sqrt(image_count);
        let rows = image_count.div_ceil(cols);
        let last_filled_row = (image_count - 1) / cols;
        let last_row_items = image_count - last_filled_row * cols;
        let last_row_shift_px = (cols - last_row_items) * slot_w / 2;
        Self { cols, rows, slot_w, slot_h, last_filled_row, last_row_items, last_row_shift_px }
    }

    const fn canvas_width(&self) -> usize {
        self.cols * self.slot_w
    }

    const fn canvas_height(&self) -> usize {
        self.rows * self.slot_h
    }

    /// (``image_index``, ``x_in_slot``, ``y_in_slot``) for a canvas pixel,
    /// or `None` for padding / empty slots.
    fn cell_at(&self, x: usize, y: usize) -> Option<(usize, u32, u32)> {
        let slot_row = y / self.slot_h;
        let y_in_slot = y % self.slot_h;

        let (slot_col, x_in_slot) = if slot_row == self.last_filled_row && self.last_row_shift_px > 0 {
            let x_shifted = x.checked_sub(self.last_row_shift_px)?; // left padding
            let shifted_col = x_shifted / self.slot_w;
            if shifted_col >= self.last_row_items {
                return None; // right padding
            }
            (shifted_col, x_shifted % self.slot_w)
        } else {
            (x / self.slot_w, x % self.slot_w)
        };

        Some((slot_row * self.cols + slot_col, x_in_slot as u32, y_in_slot as u32))
    }
}

fn load_rgba(path: &Path) -> Option<RgbaImage> {
    ImageReader::open(path).and_then(|reader| reader.decode().map_err(std::io::Error::other)).map_err(|e| eprintln!("skipping {}: {e}", path.display())).ok().map(|img| img.to_rgba8())
}

fn render_grid(images: &[RgbaImage]) -> RgbaImage {
    let (max_w, max_h) = images.iter().map(image::ImageBuffer::dimensions).reduce(|(w1, h1), (w2, h2)| (w1.max(w2), h1.max(h2))).expect("no images");
    let grid = Grid::new(images.len(), max_w as usize, max_h as usize);

    let canvas_w = grid.canvas_width();
    let width = u32::try_from(canvas_w).expect("canvas too wide");
    let height = u32::try_from(grid.canvas_height()).expect("canvas too tall");
    let mut canvas = RgbaImage::new(width, height);

    canvas.par_pixels_mut().enumerate().for_each(|(pixel_index, pixel_value)| {
        let x = pixel_index % canvas_w;
        let y = pixel_index / canvas_w;
        let Some((index, x_in_slot, y_in_slot)) = grid.cell_at(x, y) else { return };
        let Some(img) = images.get(index) else { return };

        // center the image within its slot instead of pinning it top-left
        let offset_x = (grid.slot_w as u32 - img.width()) / 2;
        let offset_y = (grid.slot_h as u32 - img.height()) / 2;

        // margin around the centered image -> stays transparent
        let Some(px) = x_in_slot.checked_sub(offset_x) else { return };
        let Some(py) = y_in_slot.checked_sub(offset_y) else { return };

        if px < img.width() && py < img.height() {
            *pixel_value = *img.get_pixel(px, py);
        }
    });
    canvas
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();

    if args.iter().any(|a| a == "--help") {
        let exe = std::env::current_exe().ok().and_then(|p| p.file_name().and_then(|n| n.to_str().map(String::from))).unwrap_or_else(|| String::from("imgrid"));
        println!("Usage: {exe} [files]");
        println!("Generates a square grid with all given images, saved as \"grid.png\"");
        return Ok(());
    }

    let valid_paths: Vec<PathBuf> = args[1..].iter().map(PathBuf::from).filter(|p| p.exists()).collect();

    if valid_paths.is_empty() {
        eprintln!("No valid images given. Exiting...");
        exit(0);
    }

    let images: Vec<RgbaImage> = valid_paths.iter().filter_map(|p| load_rgba(p)).collect();

    let grid_image_count = images.len();
    if grid_image_count < 2 {
        eprintln!("Need at least two decodable images (got {grid_image_count}). Exiting...");
        exit(1);
    }

    let (max_w, max_h) = images.iter().map(image::ImageBuffer::dimensions).reduce(|(w1, h1), (w2, h2)| (w1.max(w2), h1.max(h2))).expect("no valid images");
    let grid = Grid::new(grid_image_count, max_w as usize, max_h as usize);
    println!("Grid size: {} x {}", grid.rows, grid.cols);
    println!("Canvas: {} x {} px", grid.canvas_width(), grid.canvas_height());

    let canvas = render_grid(&images);
    canvas.save("grid.png")?;
    println!("wrote grid.png");
    Ok(())
}

#[cfg(test)]
mod tests {
    use image::Rgba;

    use super::*;

    const COLORS: [(u8, u8, u8); 5] = [
        (255, 0, 0),   // red
        (0, 255, 0),   // green
        (0, 0, 255),   // blue
        (255, 255, 0), // yellow
        (255, 0, 255), // magenta
    ];
    const TRANSPARENT: Rgba<u8> = Rgba([0, 0, 0, 0]);

    fn fixtures(count: usize) -> Vec<RgbaImage> {
        fixtures_with_size::<u32>(count, 4, 4)
    }

    fn fixtures_with_size<U: Into<u32>>(count: usize, width: U, height: U) -> Vec<RgbaImage> {
        let (width, height) = (width.into(), height.into());
        COLORS[..count].iter().map(|&(r, g, b)| RgbaImage::from_pixel(width, height, Rgba([r, g, b, 255]))).collect()
    }

    fn color(index: usize) -> Rgba<u8> {
        let (r, g, b) = COLORS[index];
        Rgba([r, g, b, 255])
    }

    #[test]
    fn two_unequal_sized_images() {
        let mut images = fixtures_with_size::<u32>(1, 2, 2);
        let mut tmp = fixtures_with_size::<u32>(1, 4, 4);
        images.append(&mut tmp);
        assert_eq!(tmp, [] as [image::ImageBuffer<image::Rgba<u8>, std::vec::Vec<u8>>; 0]);
        let canvas = render_grid(&images);
        assert_eq!(canvas.dimensions(), (8, 4));
        assert_eq!(*canvas.get_pixel(0, 0), TRANSPARENT);
        assert_eq!(*canvas.get_pixel(0, 1), TRANSPARENT);
        assert_eq!(*canvas.get_pixel(0, 2), TRANSPARENT);
        assert_eq!(*canvas.get_pixel(0, 3), TRANSPARENT);
        assert_eq!(*canvas.get_pixel(1, 0), TRANSPARENT);
        assert_eq!(*canvas.get_pixel(1, 3), TRANSPARENT);
        assert_eq!(*canvas.get_pixel(2, 0), TRANSPARENT);
        assert_eq!(*canvas.get_pixel(2, 3), TRANSPARENT);
        assert_eq!(*canvas.get_pixel(3, 0), TRANSPARENT);
        assert_eq!(*canvas.get_pixel(3, 1), TRANSPARENT);
        assert_eq!(*canvas.get_pixel(3, 2), TRANSPARENT);
        assert_eq!(*canvas.get_pixel(3, 3), TRANSPARENT);
        assert_eq!(*canvas.get_pixel(1, 1), color(0));
        assert_eq!(*canvas.get_pixel(1, 2), color(0));
        assert_eq!(*canvas.get_pixel(2, 1), color(0));
        assert_eq!(*canvas.get_pixel(2, 2), color(0));
        let res = canvas.save("two_unequal_sized_images.bmp");
        assert!(res.is_ok());
    }

    #[test]
    fn two_images_side_by_side() {
        let grid = Grid::new(2, 4, 4);
        assert_eq!((grid.cols, grid.rows), (2, 1));
        assert_eq!((grid.canvas_width(), grid.canvas_height()), (8, 4));

        let canvas = render_grid(&fixtures(2));
        assert_eq!(canvas.dimensions(), (8, 4));
        assert_eq!(*canvas.get_pixel(0, 0), color(0));
        assert_eq!(*canvas.get_pixel(3, 3), color(0));
        assert_eq!(*canvas.get_pixel(4, 0), color(1));
        assert_eq!(*canvas.get_pixel(7, 3), color(1));
    }

    #[test]
    fn three_images_center_the_last_row() {
        let grid = Grid::new(3, 4, 4);
        assert_eq!((grid.cols, grid.rows), (2, 2));
        assert_eq!(grid.last_row_shift_px, 2); // (2 - 1) * 4 / 2

        let canvas = render_grid(&fixtures(3));
        assert_eq!(canvas.dimensions(), (8, 8));
        // full first row
        assert_eq!(*canvas.get_pixel(0, 0), color(0));
        assert_eq!(*canvas.get_pixel(4, 0), color(1));
        assert_eq!(*canvas.get_pixel(7, 3), color(1));
        // last row: single image centered at x in [2, 6)
        assert_eq!(*canvas.get_pixel(2, 4), color(2));
        assert_eq!(*canvas.get_pixel(5, 7), color(2));
        // padding left and right of it
        assert_eq!(*canvas.get_pixel(0, 4), TRANSPARENT);
        assert_eq!(*canvas.get_pixel(1, 4), TRANSPARENT);
        assert_eq!(*canvas.get_pixel(6, 4), TRANSPARENT);
        assert_eq!(*canvas.get_pixel(7, 4), TRANSPARENT);
    }

    #[test]
    fn four_images_fill_a_square() {
        let grid = Grid::new(4, 4, 4);
        assert_eq!((grid.cols, grid.rows), (2, 2));
        assert_eq!(grid.last_row_shift_px, 0);

        let canvas = render_grid(&fixtures(4));
        assert_eq!(canvas.dimensions(), (8, 8));
        // row-major: 0 1 / 2 3
        assert_eq!(*canvas.get_pixel(1, 1), color(0));
        assert_eq!(*canvas.get_pixel(5, 1), color(1));
        assert_eq!(*canvas.get_pixel(1, 5), color(2));
        assert_eq!(*canvas.get_pixel(5, 5), color(3));
        // no transparent padding anywhere
        assert_eq!(*canvas.get_pixel(0, 0), color(0));
        assert_eq!(*canvas.get_pixel(7, 7), color(3));
    }

    #[test]
    fn five_images_center_last_row_at_half_slot_precision() {
        let grid = Grid::new(5, 4, 4);
        assert_eq!((grid.cols, grid.rows), (3, 2));
        assert_eq!(grid.last_row_shift_px, 2); // (3 - 2) * 4 / 2

        let canvas = render_grid(&fixtures(5));
        assert_eq!(canvas.dimensions(), (12, 8));
        // full first row of three
        assert_eq!(*canvas.get_pixel(1, 1), color(0));
        assert_eq!(*canvas.get_pixel(5, 1), color(1));
        assert_eq!(*canvas.get_pixel(9, 1), color(2));
        // last row: images 3 and 4 at x in [2, 6) and [6, 10)
        assert_eq!(*canvas.get_pixel(2, 4), color(3));
        assert_eq!(*canvas.get_pixel(5, 7), color(3));
        assert_eq!(*canvas.get_pixel(6, 4), color(4));
        assert_eq!(*canvas.get_pixel(9, 7), color(4));
        // 2 px padding on each side
        assert_eq!(*canvas.get_pixel(0, 4), TRANSPARENT);
        assert_eq!(*canvas.get_pixel(1, 4), TRANSPARENT);
        assert_eq!(*canvas.get_pixel(10, 4), TRANSPARENT);
        assert_eq!(*canvas.get_pixel(11, 4), TRANSPARENT);
    }
}
