#version 330 core

uniform vec2 u_view_size;

layout (location = 0) in vec2 v_pos;
layout (location = 1) in vec2 v_tex_coord;

out vec2 f_tex_coord;
out vec2 f_pos;

void main() {
    f_tex_coord = v_tex_coord;
    f_pos = v_pos;
    gl_Position = vec4(2.0 * v_pos.x / u_view_size.x - 1.0, 1.0 - 2.0 * v_pos.y / u_view_size.y, 0, 1);
}
