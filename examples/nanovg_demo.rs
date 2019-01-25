use glutin::*;
use rdraw::*;

mod support;

use support::rdraw_gl::*;

fn render_demo(renderer: &mut GlCanvasRenderer, canvas: &mut Canvas, width: f32, height: f32, t: f32) {
    renderer.clear(76, 76, 76, 255);

    draw_lines(renderer, canvas, 120.0, height - 50.0, 600.0, 50.0, t);

    draw_widths(renderer, canvas, 10.0, 50.0, 30.0);
}

fn draw_lines(renderer: &mut GlCanvasRenderer, canvas: &mut Canvas, x: f32, y: f32, w: f32, _h: f32, t: f32) {
    let pad = 5.0;
    let s = w / 9.0 - pad * 2.0;
    let joins = [LineJoin::Miter, LineJoin::Round, LineJoin::Bevel];
    let caps = [LineCap::Butt, LineCap::Round, LineCap::Square];

    let pts = [
        -s * 0.25 + (t * 0.3).cos() * s * 0.5,
        (t * 0.3).sin() * s * 0.5,
        -s * 0.25,
        0.0,
        s * 0.25,
        0.0,
        s * 0.25 + (-t * 0.3).cos() * s * 0.5,
        (-t * 0.3).sin() * s * 0.5,
    ];

    for (i,join) in joins.iter().enumerate() {
        for (j, cap) in caps.iter().enumerate() {
            let fx = x + s * 0.5 + (i * 3 + j) as f32 / 9.0 * w + pad;
            let fy = y - s * 0.5 + pad;

            canvas.set_line_cap(*cap);
            canvas.set_line_join(*join);

            canvas.set_line_width(s * 0.3);
            canvas.set_stroke_color(Color::rgba(0, 0, 0, 160));

            canvas.begin_path()
                .move_to(fx + pts[0], fy + pts[1])
                .line_to(fx + pts[2], fy + pts[3])
                .line_to(fx + pts[4], fy + pts[5])
                .line_to(fx + pts[6], fy + pts[7])
                .stroke(renderer);

            canvas.set_line_cap(LineCap::Butt);
            canvas.set_line_join(LineJoin::Miter);

            canvas.set_line_width(1.0);
            canvas.set_stroke_color(Color::rgba(0, 192, 255, 255));
            canvas.begin_path()
                .move_to(fx + pts[0], fy + pts[1])
                .line_to(fx + pts[2], fy + pts[3])
                .line_to(fx + pts[4], fy + pts[5])
                .line_to(fx + pts[6], fy + pts[7])
                .stroke(renderer);
        }
    }
}

fn draw_widths(renderer: &mut GlCanvasRenderer, canvas: &mut Canvas, x: f32, mut y: f32, length: f32) {
    canvas.set_stroke_color(Color::rgba(0, 0, 0, 255));

    for i in 0..20 {
        let width = (i as f32 + 0.5) * 0.1;
        canvas.set_line_width(width);
        canvas.begin_path()
            .move_to(x, y)
            .line_to(x + length, y + length * 0.3)
            .stroke(renderer);
        y += 10.0;
    }
}

fn main() {
    let mut events_loop = EventsLoop::new();
    let window = WindowBuilder::new()
        .with_title("NanoVG")
        .with_dimensions((1000, 600).into());

    let context = ContextBuilder::new()
        .with_vsync(true);
    let gl_window = GlWindow::new(window, context, &events_loop).unwrap();

    unsafe {
        gl_window.make_current().unwrap();
    }

    gl::load_with(|symbol| gl_window.get_proc_address(symbol) as *const _);

    let mut renderer =  GlCanvasRenderer::new();
    let mut canvas = Canvas::new();

    let mut running = true;
    let mut t = 0.0;
    while running {
        events_loop.poll_events(|event| {
            match event {
                Event::WindowEvent{ event, .. } => match event {
                    WindowEvent::CloseRequested => running = false,
                    WindowEvent::KeyboardInput { input, .. } => if input.virtual_keycode == Some(VirtualKeyCode::Q) && input.modifiers.logo {
                        running = false
                    },
                    WindowEvent::Resized(logical_size) => {
                        let dpi_factor = gl_window.get_hidpi_factor();
                        let size = logical_size.to_physical(dpi_factor);
                        gl_window.resize(size);
                        renderer.set_viewport_size(logical_size.width as f32, logical_size.height as f32);
                    },
                    _ => ()
                },
                _ => ()
            }
        });

        if let Some(logical_size) = gl_window.get_inner_size() {
            let width = logical_size.width as f32;
            let height = logical_size.height as f32;
            renderer.set_viewport_size(width, height);
            render_demo(&mut renderer, &mut canvas, width, height, t);
        }

        gl_window.swap_buffers().unwrap();

        t += 0.0016;
    }
}