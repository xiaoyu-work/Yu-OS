//! Yu Agent speaking mark. While the agent responds, the brand symbol's three
//! bars shrink to 54% and back (periods 0.9 s, 0.7 s and 1.1 s) beside the
//! still blue dot, matching the Web and terminal clients.

use std::sync::LazyLock;
use std::time::{Duration, Instant};

use cosmic::iced::widget::canvas::{self, Canvas, Event, Frame, Geometry, LineCap, Path, Stroke};
use cosmic::iced::{Color, Length, Point, Rectangle, mouse, window};
use cosmic::{Element, Renderer, Theme};

static EPOCH: LazyLock<Instant> = LazyLock::new(Instant::now);

/// Bars in the 620 × 600 symbol grid: x1, y1, x2, y2, period and lead seconds.
const BARS: [[f32; 6]; 3] = [
    [64.0, 514.0, 298.0, 70.0, 0.9, 0.0],
    [230.0, 514.0, 381.0, 226.0, 0.7, 0.25],
    [388.0, 514.0, 483.0, 334.0, 1.1, 0.5],
];
const BRAND_BLUE: Color = Color::from_rgb8(0, 92, 254);
const FRAME_INTERVAL: Duration = Duration::from_millis(33);

struct Speaking;

impl<Message> canvas::Program<Message, Theme, Renderer> for Speaking {
    type State = ();

    fn update(
        &self,
        _state: &mut (),
        event: &Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        match event {
            Event::Window(window::Event::RedrawRequested(now)) => {
                Some(canvas::Action::request_redraw_at(*now + FRAME_INTERVAL))
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry<Renderer>> {
        let mut frame = Frame::new(renderer, bounds.size());
        let unit = (bounds.width / 620.0).min(bounds.height / 600.0);
        let left = (bounds.width - 620.0 * unit) / 2.0;
        let top = (bounds.height - 600.0 * unit) / 2.0;
        let at = |x: f32, y: f32| Point::new(left + x * unit, top + y * unit);
        let seconds = EPOCH.elapsed().as_secs_f32();
        let ink = Color::from(theme.cosmic().on_bg_color());
        for [x1, y1, x2, y2, period, lead] in BARS {
            let phase = ((seconds + lead) / period).fract();
            let length = 1.0 - 0.23 * (1.0 - (std::f32::consts::TAU * phase).cos());
            frame.stroke(
                &Path::line(
                    at(x1, y1),
                    at(x1 + (x2 - x1) * length, y1 + (y2 - y1) * length),
                ),
                Stroke::default()
                    .with_color(ink)
                    .with_width(66.0 * unit)
                    .with_line_cap(LineCap::Round),
            );
        }
        frame.fill(&Path::circle(at(553.0, 520.0), 39.0 * unit), BRAND_BLUE);
        vec![frame.into_geometry()]
    }
}

pub(crate) fn speaking<'a, Message: 'a>(size: f32) -> Element<'a, Message> {
    Canvas::new(Speaking)
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .into()
}
