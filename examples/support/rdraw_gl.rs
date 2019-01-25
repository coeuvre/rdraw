use std::{
    mem::*,
    ffi::*,
    ptr::*,
    rc::Rc,
};
use gl::types::*;

use rdraw::*;

pub struct GlCanvasRenderer {
    width: f32,
    height: f32,
    render_triangle_shader: RenderTriangleShader,
}

impl GlCanvasRenderer {
    pub fn new() -> GlCanvasRenderer {
        unsafe {
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);

            gl::Enable(gl::SCISSOR_TEST);
        }

        GlCanvasRenderer {
            width: 0.0,
            height: 0.0,
            render_triangle_shader: RenderTriangleShader::load(),
        }
    }

    pub fn set_viewport_size(&mut self, width: f32, height: f32) {
        self.width = width;
        self.height = height;
        unsafe {
            gl::Viewport(0, 0, self.width as i32, self.height as i32);
            gl::Scissor(0, 0, self.width as i32, self.height as i32);
        }
    }

    pub fn clear(&mut self, r: u8, g: u8, b: u8, a: u8) {
        unsafe {
            gl::ClearColor(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }
    }
}

impl CanvasRenderer for GlCanvasRenderer {}

impl StrokeRenderer for GlCanvasRenderer {
    fn render(&mut self, stroke: Stroke) {
        let nverts = stroke.path_iter().map(|path| path.verts().len()).sum();
        let nindices = (nverts - 2) * 3;
        let mut verts = Vec::with_capacity(nverts);
        let mut indices = Vec::with_capacity(nindices);

        let transform_pos = |pos: [f32; 2]| {
            let x = pos[0];
            let y = self.height - pos[1];
            let x = x / self.width * 2.0 - 1.0;
            let y = y / self.height * 2.0 - 1.0;
            [x, y]
        };

        let color = stroke.color();

        for path in stroke.path_iter() {
            let offset = verts.len();
            let ntriangles = path.verts().len();
            verts.extend(path.verts().iter().map(|vert| {
                Vertex {
                    pos: transform_pos([vert.x, vert.y]),
                    tex_coord: [vert.u, vert.v],
                    color: [color.r as f32 / 255.0, color.g as f32 / 255.0, color.b as f32 / 255.0, color.a as f32 / 255.0],
                }
            }));

            for index in 0..(ntriangles - 2) {
                indices.push((index + offset) as u32);
                indices.push((index + 1 + offset) as u32);
                indices.push((index + 2 + offset) as u32);
            }
        }

        self.render_triangle_shader.upload_data(&verts, &indices);
        self.render_triangle_shader.render(0, indices.len());
    }
}

macro_rules! offset_of {
    ($ty:ty, $field:tt) => ({
        let base = std::ptr::null::<$ty>();
        let field = &(*base).$field as *const _;
        field as usize - base as usize
    });
}

#[repr(C)]
#[derive(Clone)]
struct Vertex {
    pub pos: [f32; 2],
    pub tex_coord: [f32; 2],
    pub color: [f32; 4],
}

type Index = u32;

struct RenderTriangleShader {
    program: GLprogram,
    vao: GLuint,
    vbo: GLuint,
    ebo: GLuint,
}

static RENDER_TRIANGLE_VERTEX_SHADER: &str = r#"
#version 330 core

layout (location = 0) in vec2 attrib_pos;
layout (location = 1) in vec2 attrib_tex_coord;
layout (location = 2) in vec4 attrib_color;

out vec2 vertex_tex_coord;
out vec4 vertex_color;

void main()
{
    gl_Position = vec4(attrib_pos, 0, 1);
    vertex_tex_coord = attrib_tex_coord;
    vertex_color = attrib_color;
}
"#;

static RENDER_TRIANGLE_FRAGMENT_SHADER: &str = r#"
#version 330 core

in vec2 vertex_tex_coord;
in vec4 vertex_color;

out vec4 frag_color;

void main()
{
    frag_color = vertex_color;
}
"#;

impl RenderTriangleShader {
    pub fn load() -> RenderTriangleShader {
        let mut program = GLprogram::new();
        let mut vs = GLshader::new(gl::VERTEX_SHADER);
        vs.compile(RENDER_TRIANGLE_VERTEX_SHADER);
        program.attach(&vs);

        let mut fs = GLshader::new(gl::FRAGMENT_SHADER);
        fs.compile(RENDER_TRIANGLE_FRAGMENT_SHADER);
        program.attach(&fs);

        program.link();

        let mut vao = 0;
        let mut vbo = 0;
        let mut ebo = 0;

        unsafe {
            gl::GenVertexArrays(1, &mut vao);
            gl::GenBuffers(1, &mut vbo);
            gl::GenBuffers(1, &mut ebo);

            gl::BindVertexArray(vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, ebo);

            gl::EnableVertexAttribArray(0);
            gl::VertexAttribPointer(
                0, 2, gl::FLOAT, gl::FALSE,
                size_of::<Vertex>() as i32,
                offset_of!(Vertex, pos) as *const _
            );

            gl::EnableVertexAttribArray(1);
            gl::VertexAttribPointer(
                1, 2, gl::FLOAT, gl::FALSE,
                size_of::<Vertex>() as i32,
                offset_of!(Vertex, tex_coord) as *const _
            );

            gl::EnableVertexAttribArray(2);
            gl::VertexAttribPointer(
                2, 4, gl::FLOAT, gl::FALSE,
                size_of::<Vertex>() as i32,
                offset_of!(Vertex, color) as *const _
            );

            gl::BindVertexArray(0);

            program.active();
        }

        RenderTriangleShader {
            program,
            vao,
            vbo,
            ebo,
        }
    }

    pub fn upload_data(&mut self, vertices: &[Vertex], indices: &[Index]) {
        unsafe {
            gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (size_of::<Vertex>() * vertices.len()) as isize,
                vertices.as_ptr() as *const _,
                gl::STREAM_DRAW,
            );

            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, self.ebo);
            gl::BufferData(
                gl::ELEMENT_ARRAY_BUFFER,
                (size_of::<Index>() * indices.len()) as isize,
                indices.as_ptr() as *const _,
                gl::STREAM_DRAW,
            );
        }
    }

    pub fn render(&mut self, start: usize, len: usize) {
        unsafe {
//            gl::PolygonMode(gl::FRONT_AND_BACK, gl::LINE);
            self.program.active();
            gl::BindVertexArray(self.vao);
            gl::DrawElements(gl::TRIANGLES, len as i32, gl::UNSIGNED_INT, start as *const _);
            gl::BindVertexArray(0);
        }
    }
}

struct GLshader {
    id: GLuint,
}

impl GLshader {
    pub fn new(ty: GLenum) -> GLshader {
        GLshader {
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
                let mut buffer = Vec::with_capacity(512);
                gl::GetShaderInfoLog(self.id, buffer.len() as i32, null_mut(), buffer.as_mut_ptr() as *mut i8);
                panic!("{}", CString::new(buffer).unwrap().to_string_lossy())
            }
        }

    }
}

impl Drop for GLshader {
    fn drop(&mut self) {
        unsafe { gl::DeleteShader(self.id) }
    }
}

struct GLprogram {
    id: GLuint,
}

impl GLprogram {
    pub fn new() -> GLprogram {
        GLprogram {
            id: unsafe { gl::CreateProgram() },
        }
    }

    pub fn attach(&mut self, shader: &GLshader) {
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
                let mut buffer = Vec::with_capacity(512);
                gl::GetProgramInfoLog(self.id, buffer.len() as i32, null_mut(), buffer.as_mut_ptr() as *mut i8);
                panic!("{}", CString::new(buffer).unwrap().to_string_lossy())
            }
        }
    }

    pub fn active(&mut self) {
        unsafe {
            gl::UseProgram(self.id);
        }
    }
}

impl Drop for GLprogram {
    fn drop(&mut self) {
        unsafe { gl::DeleteProgram(self.id); }
    }
}
