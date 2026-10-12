#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }

    pub fn inset(&self, by: f32) -> Rect {
        Rect::new(self.x + by, self.y + by, (self.w - 2.0 * by).max(0.0), (self.h - 2.0 * by).max(0.0))
    }

    pub fn scaled(&self, scale: f32, dx: f32, dy: f32) -> Rect {
        Rect::new(dx + self.x * scale, dy + self.y * scale, self.w * scale, self.h * scale)
    }
}

/// Where the fixed-size virtual screen lands inside a window of another size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Letterbox {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
}

impl Letterbox {
    pub fn fit(virtual_size: (f32, f32), physical: (f32, f32)) -> Letterbox {
        let scale = (physical.0 / virtual_size.0).min(physical.1 / virtual_size.1).max(0.0001);
        Letterbox {
            x: ((physical.0 - virtual_size.0 * scale) / 2.0).floor(),
            y: ((physical.1 - virtual_size.1 * scale) / 2.0).floor(),
            scale,
        }
    }

    pub fn to_virtual(&self, x: f32, y: f32) -> (f32, f32) {
        ((x - self.x) / self.scale, (y - self.y) / self.scale)
    }
}

/// Horizontal anchor for `show ... at <position>`, as a fraction of the free width.
pub fn xalign(position: Option<&str>) -> f32 {
    match position {
        Some("left") => 0.0,
        Some("right") => 1.0,
        _ => 0.5,
    }
}

/// A sprite scaled to the screen height, standing on the bottom edge.
pub fn sprite_rect(position: Option<&str>, image: (f32, f32), screen: (f32, f32)) -> Rect {
    let scale = screen.1 / image.1.max(1.0);
    let (w, h) = (image.0 * scale, screen.1);
    Rect::new((screen.0 - w) * xalign(position), screen.1 - h, w, h)
}

/// A background scaled to cover the whole screen, centred; overflow is cropped by the screen edge.
pub fn cover_rect(image: (f32, f32), screen: (f32, f32)) -> Rect {
    let scale = (screen.0 / image.0.max(1.0)).max(screen.1 / image.1.max(1.0));
    let (w, h) = (image.0 * scale, image.1 * scale);
    Rect::new((screen.0 - w) / 2.0, (screen.1 - h) / 2.0, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: (f32, f32) = (1280.0, 720.0);

    #[test]
    fn sprites_fill_the_screen_height_and_stand_on_the_bottom() {
        let r = sprite_rect(None, (400.0, 800.0), SCREEN);
        assert_eq!(r, Rect::new(460.0, 0.0, 360.0, 720.0));
    }

    #[test]
    fn left_and_right_sprites_touch_the_screen_edges() {
        assert_eq!(sprite_rect(Some("left"), (400.0, 800.0), SCREEN).x, 0.0);
        let right = sprite_rect(Some("right"), (400.0, 800.0), SCREEN);
        assert_eq!(right.x + right.w, 1280.0);
        assert_eq!(sprite_rect(Some("center"), (400.0, 800.0), SCREEN), sprite_rect(None, (400.0, 800.0), SCREEN));
        assert_eq!(sprite_rect(Some("somewhere"), (400.0, 800.0), SCREEN).x, 460.0);
    }

    #[test]
    fn backgrounds_cover_the_screen_and_crop_the_overflow_evenly() {
        assert_eq!(cover_rect((1920.0, 1080.0), SCREEN), Rect::new(0.0, 0.0, 1280.0, 720.0));
        let tall = cover_rect((1000.0, 1000.0), SCREEN);
        assert_eq!(tall, Rect::new(0.0, -280.0, 1280.0, 1280.0));
    }

    #[test]
    fn letterbox_centres_the_virtual_screen_and_maps_clicks_back() {
        let lb = Letterbox::fit(SCREEN, (2560.0, 1600.0));
        assert_eq!(lb, Letterbox { x: 0.0, y: 80.0, scale: 2.0 });
        assert_eq!(lb.to_virtual(1280.0, 800.0), (640.0, 360.0));
        let pillar = Letterbox::fit(SCREEN, (1000.0, 360.0));
        assert_eq!(pillar, Letterbox { x: 180.0, y: 0.0, scale: 0.5 });
    }

    #[test]
    fn rect_hit_testing_is_half_open() {
        let r = Rect::new(10.0, 10.0, 20.0, 20.0);
        assert!(r.contains(10.0, 10.0));
        assert!(!r.contains(30.0, 15.0));
        assert_eq!(r.inset(5.0), Rect::new(15.0, 15.0, 10.0, 10.0));
    }
}
