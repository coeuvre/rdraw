use std::{
    mem::*,
    ffi::*,
    ptr::*,
};
use gl::types::*;

use crate::*;

pub struct GlCanvasRenderer {
    width: f32,
    height: f32,
    pixels_per_point: f32,
    shader: Shader,
    draw_calls: Vec<DrawCall>,
    vao: GLuint,
    vbo: GLuint,
    ubo: GLuint,
    uniform_buffer: UniformBuffer,
    paths: Vec<BufferRef>,
    verts: Vec<ShaderVertex>,
}

const FRAG_BINDING: GLuint = 0;

impl GlCanvasRenderer {
    pub fn new() -> GlCanvasRenderer {
        let shader = Shader::load();

        let mut vao = 0;
        let mut vbo = 0;
        let mut ubo = 0;
        let frag_size;

        unsafe {
            gl::GenVertexArrays(1, &mut vao);
            gl::GenBuffers(1, &mut vbo);

            gl::BindVertexArray(vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);

            gl::UniformBlockBinding(shader.prog.id, shader.loc_frag, FRAG_BINDING);
            gl::GenBuffers(1, &mut ubo);
            let mut align = 0;
            gl::GetIntegerv(gl::UNIFORM_BUFFER_OFFSET_ALIGNMENT, &mut align);
            frag_size = size_of::<Uniforms>() + align as usize - size_of::<Uniforms>() % align as usize;

            gl::Finish();
        }

        GlCanvasRenderer {
            width: 0.0,
            height: 0.0,
            pixels_per_point: 1.0,
            shader,
            draw_calls: Vec::new(),
            vao,
            vbo,
            ubo,
            uniform_buffer: UniformBuffer {
                uniform_size: frag_size,
                buf: Vec::new(),
                nuniforms: 0,
            },
            paths: Vec::new(),
            verts: Vec::new(),
        }
    }

    pub fn set_viewport_size(&mut self, width: f32, height: f32, pixels_per_point: f32) {
        self.width = width;
        self.height = height;
        self.pixels_per_point = pixels_per_point;
        unsafe {
            gl::Viewport(0, 0, self.width as i32, self.height as i32);
        }
    }

    pub fn clear(&mut self, r: u8, g: u8, b: u8, a: u8) {
        unsafe {
            gl::ClearColor(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }
    }

    fn add_paths(&mut self, path: &Path) -> u32 {
        let mut vertex_offset = self.verts.len() as u32;
        let mut r = BufferRef {
            stroke_offset: 0,
            stroke_count: 0,
            fill_count: 0,
            fill_offset: 0,
        };

        if let Some(stroke) = path.stroke() {
            let nverts = stroke.len() as u32;
            self.verts.extend(stroke.iter().map(|vert| ShaderVertex {
                pos: [vert.x, vert.y],
                tex_coord: [vert.u, vert.v],
            }));
            r.stroke_offset = vertex_offset;
            r.stroke_count = nverts;
            vertex_offset += nverts;
        }

        if let Some(fill) = path.fill() {
            let nverts = fill.len() as u32;
            self.verts.extend(fill.iter().map(|vert| ShaderVertex {
                pos: [vert.x, vert.y],
                tex_coord: [vert.u, vert.v],
            }));
            r.fill_offset = vertex_offset;
            r.fill_count = nverts;
            vertex_offset += nverts;
        }

        self.paths.push(r);

        vertex_offset
    }

    fn add_verts(&mut self, x: Scalar, y: Scalar, u: Scalar, v: Scalar) {
        self.verts.push(ShaderVertex {
            pos: [x, y],
            tex_coord: [u, v],
        })
    }
}

impl CanvasRenderer for GlCanvasRenderer {
    fn stroke(&mut self, paint: &Paint, scissor: &Scissor, fringe: Scalar, line_width: Scalar, paths: Paths) {
        let mut maxverts: u32 = 0;
        let mut npaths: u32 = 0;
        for path in paths.iter() {
            maxverts += path.stroke().map(|stroke| stroke.len()).unwrap_or(0) as u32;
            npaths += 1;
        }

        let path_offset = self.paths.len() as u32;
        self.paths.reserve(npaths as _);

        self.verts.reserve(maxverts as _);

        for path in paths.iter() {
            self.add_paths(&path);
        }

        let uniform_index = self.uniform_buffer.alloc(1);
        {
            let uniforms = self.uniform_buffer.get_mut(uniform_index);
            *uniforms = unsafe { std::mem::zeroed() };
            convert_paint(uniforms, paint, scissor, line_width, fringe, -1.0);
        }

        let call = DrawCall {
            ty: DrawCallType::Stroke,
            path_offset,
            path_count: npaths,
            triangle_offset: 0,
            triangle_count: 0,
            uniform_offset: self.uniform_buffer.offset(uniform_index) as u32,
            color: convert_color(paint.inner_color),
            image: 0,
            blend_func: BlendFunc {
                src_rgb: gl::ONE,
                dst_rgb: gl::ONE_MINUS_SRC_ALPHA,
                src_alpha: gl::ONE,
                dst_alpha: gl::ONE_MINUS_SRC_ALPHA,
            },
        };

        self.draw_calls.push(call);
    }

    fn fill(&mut self, paint: &Paint, scissor: &Scissor, fringe: Scalar, bounds: [Scalar; 4], paths: Paths) {
        let ty = DrawCallType::Fill;
        let triangle_count = 4;
        let mut maxverts: u32 = 0;
        let mut npaths: u32 = 0;
        for path in paths.iter() {
            maxverts += path.stroke().map(|stroke| stroke.len()).unwrap_or(0) as u32;
            npaths += 1;
        }

        maxverts += triangle_count;

        let path_offset = self.paths.len() as u32;
        self.paths.reserve(npaths as _);

        let mut vert_offset = self.verts.len() as u32;
        self.verts.reserve(maxverts as _);

        for path in paths.iter() {
            vert_offset = self.add_paths(&path);
        }

        let triangle_offset = vert_offset;
        self.add_verts(bounds[2], bounds[3], 0.5, 1.0);
        self.add_verts(bounds[2], bounds[1], 0.5, 1.0);
        self.add_verts(bounds[0], bounds[3], 0.5, 1.0);
        self.add_verts(bounds[0], bounds[1], 0.5, 1.0);

        let uniform_index = self.uniform_buffer.alloc(2);
        {
            // Simple shader for stencil
            let uniforms = self.uniform_buffer.get_mut(uniform_index);
            *uniforms = unsafe { std::mem::zeroed() };
            uniforms.stroke_thr = -1.0;
            uniforms.ty = SHADER_SIMPLE;
        }
        {
            // Fill shader
            let uniforms = self.uniform_buffer.get_mut(uniform_index + 1);
            *uniforms = unsafe { std::mem::zeroed() };
            convert_paint(uniforms, paint, scissor, fringe, fringe, -1.0);
        }

        let call = DrawCall {
            ty,
            path_offset,
            path_count: npaths,
            triangle_offset,
            triangle_count,
            uniform_offset: self.uniform_buffer.offset(uniform_index) as u32,
            color: convert_color(paint.inner_color),
            image: 0,
            blend_func: BlendFunc {
                src_rgb: gl::ONE,
                dst_rgb: gl::ONE_MINUS_SRC_ALPHA,
                src_alpha: gl::ONE,
                dst_alpha: gl::ONE_MINUS_SRC_ALPHA,
            },
        };
        self.draw_calls.push(call);
    }
}

impl GlCanvasRenderer {
    pub fn flush(&mut self) {
        if !self.draw_calls.is_empty() {
            unsafe {
                gl::UseProgram(self.shader.prog.id);

                gl::Enable(gl::CULL_FACE);
                gl::CullFace(gl::BACK);
                gl::FrontFace(gl::CCW);
                gl::Enable(gl::BLEND);
                gl::Disable(gl::DEPTH_TEST);
                gl::Disable(gl::SCISSOR_TEST);
                gl::ColorMask(gl::TRUE, gl::TRUE, gl::TRUE, gl::TRUE);
                gl::StencilMask(0xffffffff);
                gl::StencilOp(gl::KEEP, gl::KEEP, gl::KEEP);
                gl::StencilFunc(gl::ALWAYS, 0, 0xffffffff);
                gl::ActiveTexture(gl::TEXTURE0);
                gl::BindTexture(gl::TEXTURE_2D, 0);
//                gl::PolygonMode(gl::FRONT_AND_BACK, gl::LINE);

                gl::BindBuffer(gl::UNIFORM_BUFFER, self.ubo);
                gl::BufferData(
                    gl::UNIFORM_BUFFER,
                    (self.uniform_buffer.nuniforms * self.uniform_buffer.uniform_size) as GLsizeiptr,
                    self.uniform_buffer.buf.as_ptr() as * const _,
                    gl::STREAM_DRAW
                );

                gl::BindVertexArray(self.vao);

                gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);
                gl::BufferData(
                    gl::ARRAY_BUFFER,
                    (size_of::<ShaderVertex>() * self.verts.len()) as GLsizeiptr,
                    self.verts.as_ptr() as *const _, gl::STREAM_DRAW
                );
                gl::EnableVertexAttribArray(0);
                gl::EnableVertexAttribArray(1);
                gl::VertexAttribPointer(
                    0, 2, gl::FLOAT, gl::FALSE,
                    size_of::<ShaderVertex>() as GLint,
                    0 as *const _,
                );
                gl::VertexAttribPointer(
                    1, 2, gl::FLOAT, gl::FALSE,
                    size_of::<ShaderVertex>() as GLint,
                    (2 * size_of::<f32>()) as *const _,
                );

                gl::Uniform1i(self.shader.loc_tex, 0);
                let view_size = [self.width / self.pixels_per_point, self.height / self.pixels_per_point];
                gl::Uniform2fv(self.shader.loc_view_size, 1, view_size.as_ptr());

                for draw_call in self.draw_calls.iter() {
                    draw_call.draw(&self.paths, self.ubo, self.uniform_buffer.uniform_size as _);
                }

                gl::DisableVertexAttribArray(0);
                gl::DisableVertexAttribArray(1);

                gl::BindVertexArray(0);

                gl::Disable(gl::CULL_FACE);
                gl::BindBuffer(gl::ARRAY_BUFFER, 0);
                gl::UseProgram(0);

                gl::BindTexture(gl::TEXTURE_2D, 0);
            }
        }

        self.paths.clear();
        self.verts.clear();
        self.draw_calls.clear();
        self.uniform_buffer.clear();
    }
}

fn convert_color(color: [f32; 4]) -> [f32; 4] {
    let a = color[3];
    [
        color[0] * a,
        color[1] * a,
        color[2] * a,
        a
    ]
}

fn convert_transform(t: Transform) -> [f32; 12] {
    [
        t.e[0], t.e[1], 0.0, 0.0,
        t.e[2], t.e[3], 0.0, 0.0,
        t.e[4], t.e[5], 1.0, 0.0,
    ]
}

fn convert_paint(uniforms: &mut Uniforms, paint: &Paint, scissor: &Scissor, width: Scalar, fringe: Scalar, stroke_thr: Scalar) {
    uniforms.inner_col = convert_color(paint.inner_color);
    uniforms.outer_col = convert_color(paint.outer_color);

    if scissor.extent[0] < -0.5 || scissor.extent[1] < -0.5 {
        uniforms.scissor_mat = [0.0; 12];
        uniforms.scissor_ext = [1.0, 1.0];
        uniforms.scissor_scale = [1.0, 1.0];
    } else {
        unimplemented!();
    }

    uniforms.extent = paint.extent;

    uniforms.stroke_mult = (width * 0.5 + fringe * 0.5) / fringe;
    uniforms.stroke_thr = stroke_thr;

    // TODO: Texture

    uniforms.ty = SHADER_FILL_GRADIENT;
    uniforms.radius = paint.radius;
    uniforms.feather = paint.feather;
    let inv_transform = paint.transform.inverse();

    uniforms.paint_mat = convert_transform(inv_transform);
}

struct DrawCall {
    ty: DrawCallType,
    path_offset: u32,
    path_count: u32,
    triangle_offset: u32,
    triangle_count: u32,
    uniform_offset: u32,
    color: [f32; 4],
    image: u32,
    blend_func: BlendFunc,
}

#[repr(u32)]
enum DrawCallType {
    Fill,
    ConvexFill,
    Stroke,
    Triangles,
}

impl DrawCall {
    unsafe fn draw(&self, paths: &[BufferRef], ubo: GLuint, uniform_size: u32) {
        let blend = &self.blend_func;
        gl::BlendFuncSeparate(blend.src_rgb, blend.dst_rgb, blend.src_alpha, blend.dst_alpha);
        match self.ty {
            DrawCallType::Stroke => self.stroke(paths, ubo),
            DrawCallType::Fill => self.fill(paths, ubo, uniform_size),
            _ => {},
        }
    }

    unsafe fn stroke(&self, paths: &[BufferRef], ubo: GLuint) {
        let paths = &paths[self.path_offset as usize..(self.path_offset + self.path_count) as usize];

        gl::BindBufferRange(gl::UNIFORM_BUFFER, FRAG_BINDING, ubo, self.uniform_offset as _, size_of::<Uniforms>() as _);

        // TODO: Texture
        gl::BindTexture(gl::TEXTURE_2D, 0);

        for path in paths.iter() {
            gl::DrawArrays(gl::TRIANGLE_STRIP, path.stroke_offset as _, path.stroke_count as _);
        }
    }

    unsafe fn fill(&self, paths: &[BufferRef], ubo: GLuint, uniform_size: u32) {
        let paths = &paths[self.path_offset as usize..(self.path_offset + self.path_count) as usize];

        // Draw shapes
        gl::Enable(gl::STENCIL_TEST);
        gl::StencilMask(0xff);
        gl::StencilFunc(gl::ALWAYS, 0, 0xff);
        gl::ColorMask(gl::FALSE, gl::FALSE, gl::FALSE, gl::FALSE);

        gl::BindBufferRange(gl::UNIFORM_BUFFER, FRAG_BINDING, ubo, self.uniform_offset as _, size_of::<Uniforms>() as _);
        gl::StencilOpSeparate(gl::FRONT, gl::KEEP, gl::KEEP, gl::INCR_WRAP);
        gl::StencilOpSeparate(gl::BACK, gl::KEEP, gl::KEEP, gl::DECR_WRAP);
        gl::Disable(gl::CULL_FACE);
        for path in paths.iter() {
            gl::DrawArrays(gl::TRIANGLE_FAN, path.fill_offset as _, path.fill_count as _);
        }
        gl::Enable(gl::CULL_FACE);

        // Draw anti-aliased pixels
        gl::ColorMask(gl::TRUE, gl::TRUE, gl::TRUE, gl::TRUE);

        gl::BindBufferRange(gl::UNIFORM_BUFFER, FRAG_BINDING, ubo, (self.uniform_offset + uniform_size) as _, size_of::<Uniforms>() as _);

        // Anti-alias
        {
            gl::StencilFunc(gl::EQUAL, 0x0, 0xff);
            gl::StencilOp(gl::KEEP, gl::KEEP, gl::KEEP);
            // Draw fringes
            for path in paths.iter() {
                gl::DrawArrays(gl::TRIANGLE_STRIP, path.stroke_offset as _, path.stroke_count as _);
            }
        }

        // Draw fill
        gl::StencilFunc(gl::NOTEQUAL, 0x0, 0xff);
        gl::StencilOp(gl::ZERO, gl::ZERO, gl::ZERO);
        gl::DrawArrays(gl::TRIANGLE_STRIP, self.triangle_offset as _, self.triangle_count as _);

        gl::Disable(gl::STENCIL_TEST);
    }
}

struct BlendFunc {
    src_rgb: GLenum,
    dst_rgb: GLenum,
    src_alpha: GLenum,
    dst_alpha: GLenum,
}

#[repr(C)]
struct Uniforms {
    scissor_mat: [f32; 12],
    paint_mat: [f32; 12],
    inner_col: [f32; 4],
    outer_col: [f32; 4],
    scissor_ext: [f32; 2],
    scissor_scale: [f32; 2],
    extent: [f32; 2],
    radius: f32,
    feather: f32,
    stroke_mult: f32,
    stroke_thr: f32,
    tex_type: u32,
    ty: u32,
}

const SHADER_FILL_GRADIENT: u32 = 0;
const SHADER_FILL_IMAGE: u32 = 1;
const SHADER_SIMPLE: u32 = 2;
const SHADER_IMAGE: u32 = 3;

#[derive(Debug)]
struct UniformBuffer {
    uniform_size: usize,
    buf: Vec<u8>,
    nuniforms: usize,
}

impl UniformBuffer {
    fn clear(&mut self) {
        self.nuniforms = 0;
        self.buf.clear();
    }

    fn alloc(&mut self, n: usize) -> usize {
        let nbytes = self.uniform_size * n;
        self.buf.reserve(nbytes);
        unsafe { self.buf.set_len(self.buf.len() + nbytes); }
        let offset = self.nuniforms;
        self.nuniforms += n;
        offset
    }

    fn get_mut(&mut self, index: usize) -> &mut Uniforms {
        assert!(index < self.nuniforms);
        unsafe {
            &mut *(self.buf.as_mut_ptr().offset((self.uniform_size * index) as _) as *mut Uniforms)
        }
    }

    fn offset(&self, index: usize) -> usize {
        self.uniform_size * index
    }
}

struct BufferRef {
    fill_offset: u32,
    fill_count: u32,
    stroke_offset: u32,
    stroke_count: u32,
}

#[repr(C)]
#[derive(Clone)]
struct ShaderVertex {
    pos: [f32; 2],
    tex_coord: [f32; 2],
}

struct Shader {
    prog: GlProgram,
    loc_view_size: GLint,
    loc_tex: GLint,
    loc_frag: GLuint,
}

static VERTEX_SHADER: &str = include_str!("shader.vert");
static FRAGMENT_SHADER: &str = include_str!("shader.frag");

impl Shader {
    pub fn load() -> Shader {
        let mut prog = GlProgram::new();
        let mut vs = GlShader::new(gl::VERTEX_SHADER);
        vs.compile(VERTEX_SHADER);
        prog.attach(&vs);

        let mut fs = GlShader::new(gl::FRAGMENT_SHADER);
        fs.compile(FRAGMENT_SHADER);
        prog.attach(&fs);

        prog.link();

        unsafe {
            Shader {
                loc_view_size: gl::GetUniformLocation(prog.id, CString::new("u_view_size").unwrap().as_ptr()),
                loc_tex: gl::GetUniformLocation(prog.id, CString::new("u_tex").unwrap().as_ptr()),
                loc_frag: gl::GetUniformLocation(prog.id, CString::new("u_frag").unwrap().as_ptr()) as GLuint,
                prog,
            }
        }
    }
}

struct GlShader {
    id: GLuint,
}

impl GlShader {
    pub fn new(ty: GLenum) -> GlShader {
        GlShader {
            id: unsafe { gl::CreateShader(ty) },
        }
    }

    pub fn compile<T: Into<Vec<u8>>>(&mut self, source: T) {
        let source = CString::new(source).unwrap();
        unsafe {
            gl::ShaderSource(self.id, 1, &source.as_ptr(), null());
            gl::CompileShader(self.id);

            let mut success = 0;
            gl::GetShaderiv(self.id, gl::COMPILE_STATUS, &mut success);
            if success as u8 != gl::TRUE {
                let mut len = 0;
                gl::GetShaderiv(self.id, gl::INFO_LOG_LENGTH, &mut len);
                let mut buffer = Vec::with_capacity(len as usize);
                gl::GetShaderInfoLog(self.id, len, null_mut(), buffer.as_mut_ptr() as *mut i8);
                panic!("{}", CStr::from_ptr(buffer.as_ptr()).to_str().unwrap());
            }
        }
    }
}

impl Drop for GlShader {
    fn drop(&mut self) {
        unsafe { gl::DeleteShader(self.id) }
    }
}

struct GlProgram {
    id: GLuint,
}

impl GlProgram {
    pub fn new() -> GlProgram {
        GlProgram {
            id: unsafe { gl::CreateProgram() },
        }
    }

    pub fn attach(&mut self, shader: &GlShader) {
        unsafe {
            gl::AttachShader(self.id, shader.id);
        }
    }

    pub fn link(&mut self) {
        unsafe {
            gl::LinkProgram(self.id);

            let mut success = 0;
            gl::GetProgramiv(self.id, gl::LINK_STATUS, &mut success);
            if success as u8 != gl::TRUE {
                let mut len = 0;
                gl::GetProgramiv(self.id, gl::INFO_LOG_LENGTH, &mut len);
                let mut buffer = Vec::with_capacity(len as usize);
                gl::GetProgramInfoLog(self.id, len, null_mut(), buffer.as_mut_ptr() as *mut i8);
                panic!("{}", CStr::from_ptr(buffer.as_ptr()).to_str().unwrap());
            }
        }
    }
}

impl Drop for GlProgram {
    fn drop(&mut self) {
        unsafe { gl::DeleteProgram(self.id); }
    }
}
