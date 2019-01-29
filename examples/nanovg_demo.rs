use glutin::*;
use rdraw::*;

fn render_demo(renderer: &mut GlCanvasRenderer, canvas: &mut Canvas, width: f32, height: f32, t: f32) {
    renderer.clear(76, 76, 76, 255);

    draw_graph(renderer, canvas, 0.0, height / 2.0, width, height / 2.0, t);

    draw_lines(renderer, canvas, 120.0, height - 50.0, 600.0, 50.0, t);

    draw_widths(renderer, canvas, 10.0, 50.0, 30.0);
}

fn draw_graph(renderer: &mut GlCanvasRenderer, canvas: &mut Canvas, x: f32, y: f32, w: f32, h: f32, t: f32) {
    let samples = [
        (1.0 + (t * 1.2345 + (t * 0.33457).cos() * 0.44).sin()) * 0.5,
        (1.0 + (t * 0.68363 + (t * 1.3).cos() * 1.55).sin()) * 0.5,
        (1.0 + (t * 1.1642 + (t * 0.33457).cos() * 1.24).sin()) * 0.5,
        (1.0 + (t * 0.56345 + (t * 1.63).cos() * 0.14).sin()) * 0.5,
        (1.0 + (t * 1.6245 + (t * 0.254).cos() * 0.3).sin()) * 0.5,
        (1.0 + (t * 0.345 + (t * 0.03).cos() * 0.6).sin()) * 0.5,
    ];
    let dx = w / 5.0;

    let mut sx = [0.0; 6];
    let mut sy = [0.0; 6];
    for i in 0..6 {
        sx[i] = x + i as f32 * dx;
        sy[i] = y + h * samples[i] * 0.8;
    }


    // Graph line
    canvas.begin_path()
        .move_to(sx[0], sy[0] + 2.0);

    for i in 1..6 {
        canvas.bezier_to(sx[i - 1] + dx * 0.5, sy[i - 1] + 2.0, sx[i] - dx * 0.5, sy[i] + 2.0, sx[i], sy[i] + 2.0);
    }
    canvas.set_stroke_color(Color::rgba(0, 0, 0, 32));
    canvas.set_line_width(3.0);
    canvas.stroke(renderer);

    canvas.begin_path()
        .move_to(sx[0], sy[0]);
    for i in 1..6 {
        canvas.bezier_to(sx[i - 1] + dx * 0.5, sy[i - 1], sx[i] - dx * 0.5, sy[i], sx[i], sy[i]);
    }
    canvas.set_stroke_color(Color::rgba(0, 160, 192, 255));
    canvas.set_line_width(3.0);
    canvas.stroke(renderer);
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
        .with_gl_profile(GlProfile::Core)
//        .with_multisampling(4)
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
                        gl_window.resize(logical_size.to_physical(dpi_factor));
                    },
                    _ => ()
                },
                _ => ()
            }
        });

        let dpi_factor = gl_window.get_hidpi_factor();
        let logical_size = gl_window.get_inner_size().unwrap();
        let physical_size = logical_size.to_physical(dpi_factor);
        renderer.set_viewport_size(physical_size.width as f32, physical_size.height as f32, dpi_factor as f32);
        canvas.set_pixels_per_point(dpi_factor as f32);
        render_demo(&mut renderer, &mut canvas, logical_size.width as f32, logical_size.height as f32, t);

        renderer.flush();

        gl_window.swap_buffers().unwrap();

        t += 0.016;
    }
}