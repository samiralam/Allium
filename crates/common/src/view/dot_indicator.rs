use std::collections::VecDeque;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc::Sender;

use crate::command::Command;
use crate::display::Display;
use crate::display::color::Color;
use crate::geom::{Point, Rect};
use crate::platform::{DefaultPlatform, KeyEvent, Platform};
use crate::stylesheet::{Stylesheet, StylesheetColor};
use crate::view::View;

/// Normal dot radius when all items fit or for non-edge dots.
const DOT_RADIUS: u32 = 4;
/// Radius for the second-to-last dot at an overflowing edge.
const DOT_RADIUS_MEDIUM: u32 = 3;
/// Radius for the last dot at an overflowing edge.
const DOT_RADIUS_SMALL: u32 = 2;

#[derive(Debug, Clone)]
pub struct DotIndicator {
    point: Point,
    height: u32,
    window_size: usize,
    total: usize,
    selected: usize,
    dirty: bool,
}

impl DotIndicator {
    pub fn new(point: Point, height: u32, window_size: usize) -> Self {
        Self {
            point,
            height,
            window_size,
            total: 0,
            selected: 0,
            dirty: true,
        }
    }

    pub fn set_state(&mut self, selected: usize, total: usize) {
        if self.selected != selected || self.total != total {
            self.selected = selected;
            self.total = total;
            self.dirty = true;
        }
    }

    fn dot_radius(
        &self,
        i: usize,
        visible: usize,
        has_more_above: bool,
        has_more_below: bool,
    ) -> u32 {
        // Shrink the last two dots at each edge that overflows
        if has_more_above {
            if i == 0 {
                return DOT_RADIUS_SMALL;
            }
            if i == 1 {
                return DOT_RADIUS_MEDIUM;
            }
        }
        if has_more_below {
            if i == visible - 1 {
                return DOT_RADIUS_SMALL;
            }
            if i == visible - 2 {
                return DOT_RADIUS_MEDIUM;
            }
        }
        DOT_RADIUS
    }

    fn draw_dots(
        &self,
        display: &mut <DefaultPlatform as Platform>::Display,
        active_color: Color,
        inactive_color: Color,
    ) {
        let visible = self.window_size.min(self.total);

        let window_start = if self.total <= self.window_size {
            0
        } else {
            let half = visible / 2;
            let start = self.selected.saturating_sub(half);
            start.min(self.total - visible)
        };

        let has_more_above = window_start > 0;
        let has_more_below = window_start + visible < self.total;

        let spacing = self.height as i32 / (visible as i32 + 1);
        let top_y = self.point.y - (spacing * visible as i32) / 2;

        for i in 0..visible {
            let item_index = window_start + i;
            let radius = self.dot_radius(i, visible, has_more_above, has_more_below);
            let color = if item_index == self.selected {
                active_color
            } else {
                inactive_color
            };

            let center = Point::new(self.point.x, top_y + spacing * i as i32 + spacing / 2);

            crate::display::fill_circle(&mut display.pixmap_mut(), center, radius, color);
        }
    }
}

#[async_trait(?Send)]
impl View for DotIndicator {
    fn draw(
        &mut self,
        display: &mut <DefaultPlatform as Platform>::Display,
        styles: &Stylesheet,
    ) -> Result<bool> {
        if self.total <= 1 {
            self.dirty = false;
            return Ok(false);
        }

        if !self.dirty {
            return Ok(false);
        }

        let active_color = StylesheetColor::Foreground.to_color(styles);
        let inactive_color = StylesheetColor::Disabled.to_color(styles);

        self.draw_dots(display, active_color, inactive_color);
        self.dirty = false;

        Ok(true)
    }

    fn should_draw(&self) -> bool {
        self.dirty && self.total > 1
    }

    fn set_should_draw(&mut self) {
        self.dirty = true;
    }

    async fn handle_key_event(
        &mut self,
        _event: KeyEvent,
        _commands: Sender<Command>,
        _bubble: &mut VecDeque<Command>,
    ) -> Result<bool> {
        Ok(false)
    }

    fn children(&self) -> Vec<&dyn View> {
        vec![]
    }

    fn children_mut(&mut self) -> Vec<&mut dyn View> {
        vec![]
    }

    fn bounding_box(&mut self, _styles: &Stylesheet) -> Rect {
        Rect::new(
            self.point.x - DOT_RADIUS as i32,
            self.point.y - self.height as i32 / 2,
            DOT_RADIUS * 2,
            self.height,
        )
    }

    fn set_position(&mut self, point: Point) {
        self.point = point;
        self.dirty = true;
    }
}
