mod renderer;

pub use renderer::gl::*;

pub trait CanvasRenderer: StrokeRenderer {}

pub trait StrokeRenderer {
    fn render(&mut self, stroke: Stroke);
}

#[derive(Clone)]
pub struct Scissor {
    pub transform: Transform,
    pub extent: [f32; 2],
}

#[derive(Clone)]
pub struct Paint {
    pub transform: Transform,
    pub extent: [Scalar; 2],
    pub radius: Scalar,
    pub feather: Scalar,
    pub inner_color: [f32; 4],
    pub outer_color: [f32; 4],
    pub image: i32,
}


pub struct Stroke<'a> {
    state: &'a State,
    cache: &'a PathCache,
    fringe: Scalar,
}

impl<'a> Stroke<'a> {
    pub fn path_iter(&self) -> impl Iterator<Item=Path> {
        PathIter {
            cache: self.cache,
            index: 0,
            stroke: true,
        }
    }

    #[inline(always)]
    pub fn inner_color(&self) -> [f32; 4] {
        self.state.stroke.inner_color
    }

    #[inline(always)]
    pub fn outer_color(&self) -> [f32; 4] {
        self.state.stroke.outer_color
    }

    #[inline(always)]
    pub fn line_width(&self) -> Scalar {
        self.state.line_width
    }

    #[inline(always)]
    pub fn fringe(&self) -> Scalar {
        self.fringe
    }

    #[inline(always)]
    pub fn scissor(&self) -> &Scissor {
        &self.state.scissor
    }

    pub fn paint(&self) -> &Paint {
        &self.state.stroke
    }
}

#[derive(Copy, Clone)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color { r, g, b, a }
    }
}

pub struct PathIter<'a> {
    cache: &'a PathCache,
    index: usize,
    stroke: bool,
}

impl<'a> Iterator for PathIter<'a> {
    type Item = Path<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(path) = self.cache.paths.get(self.index) {
            self.index += 1;
            let vertex_ref = if self.stroke {
                path.stroke.as_ref().unwrap()
            } else {
                path.fill.as_ref().unwrap()
            };
            Some(Path {
                path,
                verts: &self.cache.verts,
                first: vertex_ref.first,
                count: vertex_ref.count,
            })
        } else {
            None
        }
    }
}

pub struct Path<'a> {
    path: &'a PathBuilder,
    verts: &'a [Vertex],
    first: usize,
    count: usize,
}

impl<'a> Path<'a> {
    pub fn verts(&self) -> &[Vertex] {
        &self.verts[self.first..(self.first + self.count)]
    }

    pub fn closed(&self) -> bool {
        self.path.closed
    }

    pub fn convex(&self) -> bool {
        self.path.convex
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum Winding {
    CCW,
    CW,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum LineJoin {
    Round,
    Bevel,
    Miter,
}

pub struct Canvas {
    // TODO: Use a more memory efficient way to store commands?
    commands: Vec<Command>,
    state: State,
    cache: PathCache,
    pixels_per_point: Scalar,
    tess_tol: Scalar,
    dist_tol: Scalar,
    fringe: Scalar,
}

impl Canvas {
    pub fn new() -> Canvas {
        let mut canvas = Canvas {
            commands: Vec::new(),
            state: State::default(),
            cache: PathCache::new(),
            pixels_per_point: 0.0,
            tess_tol: 0.0,
            dist_tol: 0.0,
            fringe: 0.0,
        };

        canvas.set_pixels_per_point(1.0);

        canvas
    }

    pub fn set_pixels_per_point(&mut self, pixels_per_point: Scalar) {
        self.pixels_per_point = pixels_per_point;
        self.tess_tol = 0.25 / pixels_per_point;
        self.dist_tol = 0.01 / pixels_per_point;
        self.fringe = 1.0 / pixels_per_point;
    }

    pub fn set_line_width(&mut self, line_width: Scalar) {
        self.state.line_width = line_width;
    }

    pub fn set_line_cap(&mut self, line_cap: LineCap) {
        self.state.line_cap = line_cap;
    }

    pub fn set_line_join(&mut self, line_join: LineJoin) {
        self.state.line_join = line_join;
    }

    pub fn set_stroke_color(&mut self, color: Color) {
        self.state.stroke.inner_color = self.convert_color(color);
        self.state.stroke.outer_color = self.convert_color(color);
    }

    fn convert_color(&self, color: Color) -> [f32; 4] {
        [
            color.r as f32 / 255.0,
            color.g as f32 / 255.0,
            color.b as f32 / 255.0,
            color.a as f32 / 255.0,
        ]
    }

    pub fn set_shape_anti_alias(&mut self, enabled: bool) {
        self.state.shape_anti_alias = enabled;
    }

    pub fn begin_path(&mut self) -> &mut Self {
        self.commands.clear();
        self.cache.clear();
        self
    }

    pub fn move_to(&mut self, x: Scalar, y: Scalar) -> &mut Self {
        self.commands.push(Command::MoveTo(x, y));
        self
    }

    pub fn line_to(&mut self, x: Scalar, y: Scalar) -> &mut Self {
        self.commands.push(Command::LineTo(x, y));
        self
    }

    pub fn bezier_to(&mut self, cp1x: Scalar, cp1y: Scalar, cp2x: Scalar, cp2y: Scalar, x: Scalar, y: Scalar) -> &mut Self {
        self.commands.push(Command::BezierTo(cp1x, cp1y, cp2x, cp2y, x, y));
        self
    }

    pub fn close_path(&mut self) -> &mut Self {
        self.commands.push(Command::Close);
        self
    }

    pub fn path_winding(&mut self, winding: Winding) -> &mut Self {
        self.commands.push(Command::Winding(winding));
        self
    }

    pub fn stroke<R>(&mut self, renderer: &mut R) where R: StrokeRenderer {
        let mut state = self.state.clone();
        let stroke_paint = &mut state.stroke;

        let scale = 1.0; // TODO: Get average scale from transform
        let mut line_width = clamp(state.line_width * scale, 0.0, 200.0);

        if line_width < self.fringe {
            // If the stroke width is less than pixel size, use alpha to emulate coverage.
            // Since coverage is area, scale by alpha*alpha.
            let alpha = clamp(line_width / self.fringe, 0.0, 1.0);
            stroke_paint.inner_color[3] *= alpha * alpha;
            stroke_paint.outer_color[3] *= alpha * alpha;
            line_width = self.fringe;
        }

        // TODO: Apply global alpha

        self.cache.flatten_paths(self.commands.iter(), self.tess_tol, self.dist_tol);

        let fringe = if state.shape_anti_alias {
            self.fringe
        } else {
            0.0
        };
        self.cache.expand_stroke(line_width * 0.5, fringe, state.line_cap, state.line_join, state.miter_limit, self.tess_tol);

        let stroke = Stroke {
            state: &state,
            cache: &self.cache,
            fringe,
        };

        renderer.render(stroke);
    }
}

type Scalar = f32;

const PI: Scalar = std::f32::consts::PI;
const _2_PI: Scalar = 2.0 * PI;
const FRAC_1_PI: Scalar = std::f32::consts::FRAC_1_PI;

enum Command {
    MoveTo(Scalar, Scalar),
    LineTo(Scalar, Scalar),
    BezierTo(Scalar, Scalar, Scalar, Scalar, Scalar, Scalar),
    Close,
    Winding(Winding),
}

#[derive(Clone)]
struct State {
    line_width: Scalar,
    line_cap: LineCap,
    line_join: LineJoin,
    miter_limit: Scalar,
    stroke: Paint,
    shape_anti_alias: bool,
    scissor: Scissor,
}

impl Default for State {
    fn default() -> Self {
        State {
            line_width: 1.0,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            miter_limit: 10.0,
            stroke: Paint {
                transform: Transform::identity(),
                extent: [0.0; 2],
                radius: 0.0,
                feather: 1.0,
                inner_color: [0.0, 0.0, 0.0, 1.0],
                outer_color: [0.0, 0.0, 0.0, 1.0],
                image: 0,
            },
            shape_anti_alias: true,
            scissor: Scissor {
                transform: Transform::identity(),
                extent: [-1.0; 2],
            },
        }
    }
}

#[derive(Clone, Copy)]
pub struct Transform {
    pub e: [f32; 6],
}

impl Transform {
    #[inline(always)]
    pub fn identity() -> Self {
        Transform {
            e: [
                1.0, 0.0,
                0.0, 1.0,
                0.0, 0.0,
            ]
        }
    }

    pub fn inverse(&self) -> Transform {
        let det = self.e[0] as f64 * self.e[3] as f64 - self.e[2] as f64 * self.e[1] as f64;
        if det > -1e-6 && det < 1e-6 {
            return Transform::identity();
        }
        let invdet = 1.0 / det;

        Transform {
            e: [
                (self.e[3] as f64 * invdet) as f32,
                (-self.e[1] as f64 * invdet) as f32,
                (-self.e[2] as f64 * invdet) as f32,
                (self.e[0] as f64 * invdet) as f32,
                ((self.e[2] as f64 * self.e[5] as f64 - self.e[3] as f64 * self.e[4] as f64) * invdet) as f32,
                ((self.e[1] as f64 * self.e[4] as f64 - self.e[0] as f64 * self.e[5] as f64) * invdet) as f32,
            ]
        }
    }
}


struct PathBuilder {
    first: usize,
    count: usize,
    closed: bool,
    winding: Winding,
    nbevel: usize,
    convex: bool,
    stroke: Option<PathVertexRef>,
    fill: Option<PathVertexRef>,
}

struct PathVertexRef {
    first: usize,
    count: usize,
}

struct PathCache {
    points: Vec<Point>,
    verts: Vec<Vertex>,
    paths: Vec<PathBuilder>,
    bounds: [Scalar; 4],
}

impl PathCache {
    fn new() -> PathCache {
        PathCache {
            points: Vec::new(),
            verts: Vec::new(),
            paths: Vec::new(),
            bounds: [0.0, 0.0, 0.0, 0.0],
        }
    }

    fn flatten_paths<'a, T>(&mut self, iter: T, tess_tol: Scalar, dist_tol: Scalar) where T: Iterator<Item=&'a Command> {
        for command in iter {
            match *command {
                Command::MoveTo(x, y) => {
                    self.add_path();
                    self.add_point(x, y, POINT_CORNER, dist_tol);
                }
                Command::LineTo(x, y) => self.add_point(x, y, POINT_CORNER, dist_tol),
                Command::BezierTo(cp1x, cp1y, cp2x, cp2y, x, y) => {
                    if let Some(last) = self.points.last() {
                        self.tesselate_bezier(last.x,last.y, cp1x,cp1y, cp2x,cp2y, x, y, 0, POINT_CORNER, tess_tol, dist_tol);
                    }
                },
                Command::Close => self.close_path(),
                Command::Winding(winding) => self.path_winding(winding),
            }
        }

        self.bounds[0] = std::f32::MAX;
        self.bounds[1] = std::f32::MAX;
        self.bounds[2] = std::f32::MIN;
        self.bounds[3] = std::f32::MIN;

        for path in self.paths.iter_mut() {
            let mut points = &mut self.points[path.first..(path.first + path.count)];

            // If the first and last points are the same, remove the last, mark as closed path.
            let p0 = &points[path.count - 1];
            let p1 = &points[0];
            if point_equals(p0.x, p0.y, p1.x, p1.y, dist_tol) {
                path.count = path.count - 1;
                path.closed = true;
                points = &mut self.points[path.first..(path.first + path.count)];
            }

            // Enforce winding
            if path.count > 2 {
                let area = polygon_area(points);
                match path.winding {
                    Winding::CCW => {
                        if area < 0.0 {
                            points.reverse();
                        }
                    }
                    Winding::CW => {
                        if area > 0.0 {
                            points.reverse();
                        }
                    }
                }
            }

            for (p0, p1) in edges_iter_mut(points) {
                // Calculate segment direction and length
                let (dx, dy, len) = normalize(p1.x - p0.x, p1.y - p0.y);
                p0.dx = dx;
                p0.dy = dy;
                p0.len = len;

                // Update bounds
                self.bounds[0] = self.bounds[0].min(p0.x);
                self.bounds[1] = self.bounds[1].min(p0.y);
                self.bounds[2] = self.bounds[2].max(p0.x);
                self.bounds[3] = self.bounds[3].max(p0.y);
            }
        }
    }

    fn tesselate_bezier(&mut self, x1: Scalar, y1: Scalar, x2: Scalar, y2: Scalar, x3: Scalar, y3: Scalar, x4: Scalar, y4: Scalar, level: u32, flags: u32, tess_tol: Scalar, dist_tol: Scalar) {
        if level > 10 {
            return
        }

        let x12 = (x1 + x2) * 0.5;
        let y12 = (y1 + y2) * 0.5;
        let x23 = (x2 + x3) * 0.5;
        let y23 = (y2 + y3) * 0.5;
        let x34 = (x3 + x4) * 0.5;
        let y34 = (y3 + y4) * 0.5;
        let x123 = (x12 + x23) * 0.5;
        let y123 = (y12 + y23) * 0.5;

        let dx = x4 - x1;
        let dy = y4 - y1;
        let d2 = ((x2 - x4) * dy - (y2 - y4) * dx).abs();
        let d3 = ((x3 - x4) * dy - (y3 - y4) * dx).abs();

        if (d2 + d3) * (d2 + d3) < tess_tol * (dx * dx + dy * dy) {
            self.add_point(x4, y4, flags, dist_tol);
            return;
        }

        let x234 = (x23 + x34) * 0.5;
        let y234 = (y23 + y34) * 0.5;
        let x1234 = (x123 + x234) * 0.5;
        let y1234 = (y123 + y234) * 0.5;

        self.tesselate_bezier(x1, y1, x12, y12, x123, y123, x1234, y1234, level + 1, 0, tess_tol, dist_tol);
        self.tesselate_bezier(x1234, y1234, x234, y234, x34, y34, x4, y4, level + 1, flags, tess_tol, dist_tol);
    }

    fn expand_stroke(&mut self, mut w: Scalar, fringe: Scalar, line_cap: LineCap, line_join: LineJoin, miter_limit: Scalar, tess_tol: Scalar) {
        let aa = fringe;
        let (u0, u1) = if aa == 0.0 {
            // Disable the gradient used for antialiasing when antialiasing is not used.
            (0.5, 0.5)
        } else {
            (0.0, 1.0)
        };
        let ncap = curve_divs(w, PI, tess_tol);	 // Calculate divisions per half circle.

        w += aa * 0.5;

        self.calculate_joins(w, line_join, miter_limit);

        // Calculate max vertex usage.
        let mut cverts = 0;
        for path in self.paths.iter() {
            let is_loop = path.closed;
            if line_join == LineJoin::Round {
                cverts += (path.count + path.nbevel * (ncap + 2) + 1) * 2; // plus one for loop
            } else {
                cverts += (path.count + path.nbevel * 5 + 1) * 2; // plus one for loop
            }

            if !is_loop {
                // space for caps
                if line_cap == LineCap::Round {
                    cverts += (ncap * 2 + 2) * 2;
                } else {
                    cverts += (3 + 3) * 2;
                }
            }
        }

        self.verts.clear();
        self.verts.reserve(cverts);

        let verts = &mut self.verts;

        for path in self.paths.iter_mut() {
            let first = verts.len();

            let points = &self.points[path.first..(path.first + path.count)];
            let start: usize;
            let end: usize;
            let mut p0: &Point;
            let mut p1: &Point;

            path.fill = None;

            // Calculate fringe or stroke
            let is_loop = path.closed;
            let mut p1_index;

            if is_loop {
                // Looping
                p0 = &points[path.count - 1];
                p1 = &points[0];
                p1_index = 0;
                start = 0;
                end = path.count;
            } else {
                // Add cap
                p0 = &points[0];
                p1 = &points[1];
                p1_index = 1;
                start = 1;
                end = path.count - 1;

                let (dx, dy, _) = normalize(p1.x - p0.x, p1.y - p0.y);

                match line_cap {
                    LineCap::Butt => butt_cap_start(verts, p0, dx, dy, w, -aa * 0.5, aa, u0, u1),
                    LineCap::Square => butt_cap_start(verts, p0, dx, dy, w, w - aa, aa, u0, u1),
                    LineCap::Round => round_cap_start(verts, p0, dx, dy, w, ncap, aa, u0, u1),
                }
            }

            for _ in start..end {
                if (p1.flags & (POINT_BEVEL | POINT_INNER_BEVEL)) != 0 {
                    if line_join == LineJoin::Round {
                        round_join(verts, p0, p1, w, w, u0, u1, ncap, aa);
                    } else {
                        bevel_join(verts, p0, p1, w, w, u0, u1, aa);
                    }
                } else {
                    add_vert(verts, p1.x + (p1.dmx * w), p1.y + (p1.dmy * w), u0, 1.0);
                    add_vert(verts, p1.x - (p1.dmx * w), p1.y - (p1.dmy * w), u1, 1.0);
                }
                p0 = p1;
                p1_index = p1_index + 1;
                p1 = &points[p1_index];
            }

            if is_loop {
                // Loop it
                add_vert(verts, verts[0].x, verts[0].y, u0, 1.0);
                add_vert(verts, verts[1].x, verts[1].y, u1, 1.0);
            } else {
                // Add cap
                let (dx, dy, _) = normalize(p1.x - p0.x, p1.y - p0.y);
                match line_cap {
                    LineCap::Butt => butt_cap_end(verts, p1, dx, dy, w, -aa * 0.5, aa, u0, u1),
                    LineCap::Square => butt_cap_end(verts, p1, dx, dy, w, w - aa, aa, u0, u1),
                    LineCap::Round => round_cap_end(verts, p1, dx, dy, w, ncap, aa, u0, u1),
                }
            }

            path.stroke = Some(PathVertexRef { first, count: verts.len() - first });
        }
    }

    fn calculate_joins(&mut self, w: Scalar, line_join: LineJoin, miter_limit: Scalar) {
        let mut nleft = 0;
        let mut iw = 0.0;
        if w > 0.0 {
            iw = 1.0 / w;
        }

        // Calculate which joins needs extra vertices to append, and gather vertex count.
        for path in self.paths.iter_mut() {
            path.nbevel = 0;
            let points = &mut self.points[path.first..(path.first + path.count)];
            for (p0, p1) in edges_iter_mut(points) {
                let dlx0 = p0.dy;
                let dly0 = -p0.dx;
                let dlx1 = p1.dy;
                let dly1 = -p1.dx;
                // Calculate extrusions
                p1.dmx = (dlx0 + dlx1) * 0.5;
                p1.dmy = (dly0 + dly1) * 0.5;
                let dmr2 = p1.dmx * p1.dmx + p1.dmy * p1.dmy;
                if dmr2 > 1e-6 {
                    let mut scale = 1.0 / dmr2;
                    if scale > 600.0 {
                        scale = 600.0;
                    }
                    p1.dmx *= scale;
                    p1.dmy *= scale;
                }

                // Clear flags, but keep the corner.
                p1.flags = if (p1.flags & POINT_CORNER) != 0 { POINT_CORNER } else { 0 };

                // Keep track of left turns.
                let cross = p1.dx * p0.dy - p0.dx * p1.dy;
                if cross > 0.0 {
                    nleft = nleft + 1;
                    p1.flags |= POINT_LEFT;
                }

                // Calculate if we should use bevel or miter for inner join.
                let limit = (p0.len.min(p1.len) * iw).max(1.01);
                if (dmr2 * limit*limit) < 1.0 {
                    p1.flags |= POINT_INNER_BEVEL;
                }

                // Check to see if the corner needs to be beveled.
                if (p1.flags & POINT_CORNER) != 0 {
                    if (dmr2 * miter_limit * miter_limit) < 1.0 || line_join == LineJoin::Bevel || line_join == LineJoin::Round {
                        p1.flags |= POINT_BEVEL;
                    }
                }

                if (p1.flags & (POINT_BEVEL | POINT_INNER_BEVEL)) != 0 {
                    path.nbevel = path.nbevel + 1;
                }
            }

            path.convex = if nleft == path.count { true } else { false };
        }
    }

    fn add_path(&mut self) {
        let path = PathBuilder {
            first: self.points.len(),
            count: 0,
            closed: false,
            winding: Winding::CCW,
            nbevel: 0,
            convex: false,
            stroke: None,
            fill: None,
        };
        self.paths.push(path);
    }

    fn add_point(&mut self, x: Scalar, y: Scalar, flags: u32, dist_tol: Scalar) {
        if let Some(path) = self.paths.last_mut() {
            // If the incoming and last points are the same, merge them
            if path.count > 0 && self.points.len() > 0 {
                let last_point = self.points.last_mut().unwrap();
                if point_equals(last_point.x, last_point.y, x, y, dist_tol) {
                    last_point.flags = last_point.flags | flags;
                    return;
                }
            }

            let point = Point {
                x,
                y,
                dx: 0.0,
                dy: 0.0,
                len: 0.0,
                dmx: 0.0,
                dmy: 0.0,
                flags,
            };
            self.points.push(point);
            path.count = path.count + 1;
        }
    }

    fn close_path(&mut self) {
        if let Some(path) = self.paths.last_mut() {
            path.closed = true;
        }
    }

    fn path_winding(&mut self, winding: Winding) {
        if let Some(path) = self.paths.last_mut() {
            path.winding = winding;
        }
    }

    fn clear(&mut self) {
        self.points.clear();
        self.paths.clear();
    }
}

fn bevel_join(verts: &mut Vec<Vertex>, p0: &Point, p1: &Point, lw: Scalar, rw: Scalar, lu: Scalar, ru: Scalar, _fringe: Scalar) {
    let dlx0 = p0.dy;
    let dly0 = -p0.dx;
    let dlx1 = p1.dy;
    let dly1 = -p1.dx;

    if p1.flags & POINT_LEFT != 0 {
        let (lx0, ly0, lx1, ly1) = choose_bevel(p1.flags & POINT_INNER_BEVEL, p0, p1, lw);
        add_vert(verts, lx0, ly0, lu, 1.0);
        add_vert(verts, p1.x - dlx0 * rw, p1.y - dly0 * rw, ru, 1.0);

        if p1.flags & POINT_BEVEL != 0 {
            add_vert(verts, lx0, ly0, lu, 1.0);
            add_vert(verts, p1.x - dlx0 * rw, p1.y - dly0 * rw, ru, 1.0);

            add_vert(verts, lx1, ly1, lu, 1.0);
            add_vert(verts, p1.x - dlx1 * rw, p1.y - dly1 * rw, ru, 1.0);
        } else {
            let rx0 = p1.x - p1.dmx * rw;
            let ry0 = p1.y - p1.dmy * rw;

            add_vert(verts, p1.x, p1.y, 0.5, 1.0);
            add_vert(verts, p1.x - dlx0 * rw, p1.y - dly0 * rw, ru, 1.0);

            add_vert(verts, rx0, ry0, ru, 1.0);
            add_vert(verts, rx0, ry0, ru, 1.0);

            add_vert(verts, p1.x, p1.y, 0.5, 1.0);
            add_vert(verts, p1.x - dlx1 * rw, p1.y - dly1 * rw, ru, 1.0);
        }

        add_vert(verts, lx1, ly1, lu, 1.0);
        add_vert(verts, p1.x - dlx1 * rw, p1.y - dly1 * rw, ru, 1.0);
    } else {
        let (rx0, ry0, rx1, ry1) = choose_bevel(p1.flags & POINT_INNER_BEVEL, p0, p1, -rw);

        add_vert(verts, p1.x + dlx0 * lw, p1.y + dly0 * lw, lu, 1.0);
        add_vert(verts, rx0, ry0, ru, 1.0);

        if p1.flags & POINT_BEVEL != 0 {
            add_vert(verts, p1.x + dlx0 * lw, p1.y + dly0 * lw, lu, 1.0);
            add_vert(verts, rx0, ry0, ru, 1.0);

            add_vert(verts, p1.x + dlx1 * lw, p1.y + dly1 * lw, lu, 1.0);
            add_vert(verts, rx1, ry1, ru, 1.0);
        } else {
            let lx0 = p1.x + p1.dmx * lw;
            let ly0 = p1.y + p1.dmy * lw;

            add_vert(verts, p1.x + dlx0 * lw, p1.y + dly0 * lw, lu, 1.0);
            add_vert(verts, p1.x, p1.y, 0.5, 1.0);

            add_vert(verts, lx0, ly0, lu, 1.0);
            add_vert(verts, lx0, ly0, lu, 1.0);

            add_vert(verts, p1.x + dlx1 * lw, p1.y + dly1 * lw, lu, 1.0);
            add_vert(verts, p1.x, p1.y, 0.5, 1.0);
        }

        add_vert(verts, p1.x + dlx1 * lw, p1.y + dly1 * lw, lu, 1.0);
        add_vert(verts, rx1, ry1, ru, 1.0);
    }
}

fn round_join(verts: &mut Vec<Vertex>, p0: &Point, p1: &Point, lw: Scalar, rw: Scalar, lu: Scalar, ru: Scalar, ncap: usize, _fringe: Scalar) {
    let dlx0 = p0.dy;
    let dly0 = -p0.dx;
    let dlx1 = p1.dy;
    let dly1 = -p1.dx;

    if p1.flags & POINT_LEFT != 0 {
        let (lx0, ly0, lx1, ly1) = choose_bevel(p1.flags & POINT_INNER_BEVEL, p0, p1, lw);
        let a0 = (-dly0).atan2(-dlx0);
        let a1 = {
            let a1 = (-dly1).atan2(-dlx1);
            if a1 > a0 {
                a1 - _2_PI
            } else {
                a1
            }
        };

        add_vert(verts, lx0, ly0, lu, 1.0);
        add_vert(verts, p1.x - dlx0 * rw, p1.y - dly0 * rw, ru, 1.0);

        let n = clamp((((a0 - a1) * FRAC_1_PI) * (ncap as f32)).ceil() as usize, 2, ncap);
        for i in 0..n {
            let u = i as Scalar / (n - 1) as Scalar;
            let a = a0 + u * (a1 - a0);
            let rx = p1.x + a.cos() * rw;
            let ry = p1.y + a.sin() * rw;
            add_vert(verts, p1.x, p1.y, 0.5, 1.0);
            add_vert(verts, rx, ry, ru, 1.0);
        }

        add_vert(verts, lx1, ly1, lu, 1.0);
        add_vert(verts, p1.x - dlx1 * rw, p1.y - dly1 * rw, ru, 1.0);
    } else {
        let (rx0, ry0, rx1, ry1) = choose_bevel(p1.flags & POINT_INNER_BEVEL, p0, p1, -rw);

        let a0 = dly0.atan2(dlx0);
        let a1 = {
            let a1 = dly1.atan2(dlx1);
            if a1 < a0 {
                a1 + _2_PI
            } else {
                a1
            }
        };

        add_vert(verts, p1.x + dlx0*rw, p1.y + dly0*rw, lu, 1.0);
        add_vert(verts, rx0, ry0, ru, 1.0);

        let n = clamp((((a1 - a0) / PI) * (ncap as f32)).ceil() as usize, 2, ncap);
        for i in 0..n {
            let u = i as Scalar / (n - 1) as Scalar;
            let a = a0 + u * (a1 - a0);
            let lx = p1.x + a.cos() * lw;
            let ly = p1.y + a.sin() * lw;
            add_vert(verts, lx, ly, lu, 1.0);
            add_vert(verts, p1.x, p1.y, 0.5, 1.0);
        }

        add_vert(verts, p1.x + dlx1*rw, p1.y + dly1*rw, lu, 1.0);
        add_vert(verts, rx1, ry1, ru, 1.0);
    }
}

#[inline(always)]
fn clamp<T>(a: T, mn: T, mx: T) -> T where T: PartialOrd {
    if a < mn {
        mn
    } else {
        if a > mx {
            mx
        } else {
            a
        }
    }
}

#[inline(always)]
fn choose_bevel(bevel: u32, p0: &Point, p1: &Point, w: Scalar) -> (Scalar, Scalar, Scalar, Scalar) {
    if bevel != 0 {
        (p1.x + p0.dy * w, p1.y - p0.dx * w, p1.x + p1.dy * w, p1.y - p1.dx * w)
    } else {
        (p1.x + p1.dmx * w, p1.y + p1.dmy * w, p1.x + p1.dmx * w, p1.y + p1.dmy * w)
    }
}

#[derive(Clone)]
pub struct Vertex {
    pub x: Scalar,
    pub y: Scalar,
    pub u: Scalar,
    pub v: Scalar,
}

struct Point {
    x: Scalar,
    y: Scalar,
    dx: Scalar,
    dy: Scalar,
    len: Scalar,
    dmx: Scalar,
    dmy: Scalar,
    flags: u32,
}

const POINT_CORNER: u32 = 0x01;
const POINT_LEFT: u32 = 0x02;
const POINT_BEVEL: u32 = 0x04;
const POINT_INNER_BEVEL: u32 = 0x08;

#[inline(always)]
fn point_equals(x1: Scalar, y1: Scalar, x2: Scalar, y2: Scalar, tol: Scalar) -> bool {
    let dx = x2 - x1;
    let dy = y2 - y1;
    return dx * dx + dy * dy < tol * tol;
}

fn polygon_area(points: &[Point]) -> Scalar {
    let mut area = 0.0;
    let a = &points[0];
    for i in 2..points.len() {
        let b = &points[i - 1];
        let c = &points[i];
        area += triangle_area2(a.x, a.y, b.x, b.y, c.x, c.y);
    }

    area * 0.5
}

#[inline(always)]
fn triangle_area2(ax: Scalar, ay: Scalar, bx: Scalar, by: Scalar, cx: Scalar, cy: Scalar) -> Scalar {
    let abx = bx - ax;
    let aby = by - ay;
    let acx = cx - ax;
    let acy = cy - ay;
    return acx * aby - abx * acy;
}

fn normalize(x: Scalar, y: Scalar) -> (Scalar, Scalar, Scalar)  {
    let len = (x * x + y * y).sqrt();
    let mut nx = x;
    let mut ny = y;
    if len > 1e-6 {
        let inv_len = 1.0 / len;
        nx = nx * inv_len;
        ny = ny * inv_len;
    }
    (nx, ny, len)
}

fn curve_divs(r: Scalar, arc: Scalar, tol: Scalar) -> usize {
    let da = (r / (r + tol)).acos() * 2.0;
    ((arc / da).ceil() as usize).max(2)
}

fn butt_cap_start(verts: &mut Vec<Vertex>, p: &Point, dx: Scalar, dy: Scalar, w: Scalar, d: Scalar, aa: Scalar, u0: Scalar, u1: Scalar) {
    let px = p.x - dx * d;
    let py = p.y - dy * d;
    let dlx = dy;
    let dly = -dx;
    add_vert(verts, px + dlx * w - dx * aa, py + dly * w - dy * aa, u0, 0.0);
    add_vert(verts, px - dlx * w - dx * aa, py - dly * w - dy * aa, u1, 0.0);
    add_vert(verts, px + dlx * w, py + dly * w, u0, 1.0);
    add_vert(verts, px - dlx * w, py - dly * w, u1, 1.0);
}

fn round_cap_start(verts: &mut Vec<Vertex>, p: &Point, dx: Scalar, dy: Scalar, w: Scalar, ncap: usize, _aa: Scalar, u0: Scalar, u1: Scalar) {
    let px = p.x;
    let py = p.y;
    let dlx = dy;
    let dly = -dx;

    for i in 0..ncap {
        let a = i as Scalar / (ncap - 1) as Scalar * PI;
        let ax = a.cos() * w;
        let ay = a.sin() * w;
        add_vert(verts, px - dlx * ax - dx * ay, py - dly * ax - dy * ay, u0, 1.0);
        add_vert(verts, px, py, 0.5, 1.0);
    }

    add_vert(verts, px + dlx * w, py + dly * w, u0, 1.0);
    add_vert(verts, px - dlx * w, py - dly * w, u1, 1.0);
}

fn butt_cap_end(verts: &mut Vec<Vertex>, p: &Point, dx: Scalar, dy: Scalar, w: Scalar, d: Scalar, aa: Scalar, u0: Scalar, u1: Scalar) {
    let px = p.x + dx * d;
    let py = p.y + dy * d;
    let dlx = dy;
    let dly = -dx;
    add_vert(verts, px + dlx * w, py + dly * w, u0, 1.0);
    add_vert(verts, px - dlx * w, py - dly * w, u1, 1.0);
    add_vert(verts, px + dlx * w + dx * aa, py + dly * w + dy * aa, u0, 0.0);
    add_vert(verts, px - dlx * w + dx * aa, py - dly * w + dy * aa, u1, 0.0);
}

fn round_cap_end(verts: &mut Vec<Vertex>, p: &Point, dx: Scalar, dy: Scalar, w: Scalar, ncap: usize, _aa: Scalar, u0: Scalar, u1: Scalar) {
    let px = p.x;
    let py = p.y;
    let dlx = dy;
    let dly = -dx;

    add_vert(verts, px + dlx * w, py + dly * w, u0, 1.0);
    add_vert(verts, px - dlx * w, py - dly * w, u1, 1.0);

    for i in 0..ncap {
        let a = i as Scalar / (ncap - 1) as Scalar * PI;
        let ax = a.cos() * w;
        let ay = a.sin() * w;
        add_vert(verts, px, py, 0.5, 1.0);
        add_vert(verts, px - dlx * ax + dx * ay, py - dly * ax + dy * ay, u0, 1.0);
    }
}

#[inline(always)]
fn add_vert(verts: &mut Vec<Vertex>, x: Scalar, y: Scalar, u: Scalar, v: Scalar) {
    verts.push(Vertex {
        x, y, u, v
    });
}

struct EdgeIterMut<'a, T> {
    points: &'a mut [T],
    from: usize,
    to: usize,
}

impl<'a, T> Iterator for EdgeIterMut<'a, T> {
    type Item = (&'a mut T, &'a mut T);

    fn next(&mut self) -> Option<Self::Item> {
        if self.to < self.points.len() {
            let result = unsafe {
                (&mut *(&mut self.points[self.from] as *mut T), &mut *(&mut self.points[self.to] as *mut T))
            };
            self.from = self.to;
            self.to = self.to + 1;
            Some(result)
        } else {
            None
        }
    }
}

fn edges_iter_mut<T>(points: &mut [T]) -> EdgeIterMut<T> {
    EdgeIterMut {
        from: points.len() - 1,
        to: 0,
        points,
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_edges_iter_mut() {
        let mut points = vec![100.0, 200.0, 300.0];
        let edges = edges_iter_mut(points.as_mut())
            .map(|(from, to)| (*from, *to))
            .collect::<Vec<_>>();
        assert_eq!(edges, vec![(300.0, 100.0), (100.0, 200.0), (200.0, 300.0)]);
    }
}
