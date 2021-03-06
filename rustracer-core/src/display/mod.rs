#[cfg(feature = "display")]
extern crate minifb;

use Point2i;
use film::Film;

pub trait DisplayUpdater {
    fn update(&mut self, film: &Film);
}

pub struct MinifbDisplayUpdater {
    #[cfg(feature = "display")]
    window: minifb::Window,
}

impl MinifbDisplayUpdater {
    #[cfg(feature = "display")]
    pub fn new(res: Point2i) -> MinifbDisplayUpdater {
        MinifbDisplayUpdater {
            window: minifb::Window::new("Rustracer",
                                        res.x as usize,
                                        res.y as usize,
                                        minifb::WindowOptions::default())
                    .expect("Unable to open a window"),
        }
    }

    #[cfg(not(feature = "display"))]
    pub fn new(_res: Point2i) -> MinifbDisplayUpdater {
        panic!("minifb support not compiled in!");
    }
}

// impl DisplayUpdater for MinifbDisplayUpdater {
//     #[cfg(feature = "minifb")]
//     fn update(&mut self, film: &Film) {
//         let buffer: Vec<u32> = film.render()
//             .iter()
//             .map(|p| {
//                      let rgb = p.to_srgb();
//                      (u32::from(rgb[0])) << 16 | (u32::from(rgb[1])) << 8 | (u32::from(rgb[2]))
//                  })
//             .collect();

//         self.window
//             .update_with_buffer(&buffer[..])
//             .expect("Could not update window");
//     }
//     #[cfg(not(feature = "minifb"))]
//     fn update(&mut self, film: &Film) {}
// }

// minifb::Window is not Send because of some callback it holds, but we need MinifbDisplayUpdater
// to be so we can send it to the thread collecting the tiles. This is a bit naughty but since it
// is only moved to some other thread once at the beginning, this should be fine... (I hope!)
unsafe impl Send for MinifbDisplayUpdater {}

pub struct NoopDisplayUpdater;

impl DisplayUpdater for NoopDisplayUpdater {
    fn update(&mut self, _film: &Film) {}
}
