pub mod assets;
pub mod pipeline;
pub mod types;

pub use assets::ShopAssets;
pub use pipeline::start_detection_loop;
pub use types::{CellRegions, DetectedItem, Popularity, ShopCoords};
