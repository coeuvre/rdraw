#version 330 core

#define EDGE_AA 1

layout(std140) uniform u_frag {
    mat3 scissor_mat;
    mat3 paint_mat;
    vec4 inner_col;
    vec4 outer_col;
    vec2 scissor_ext;
    vec2 scissor_scale;
    vec2 extent;
    float radius;
    float feather;
    float stroke_mult;
    float stroke_thr;
    int tex_type;
    int type;
};

uniform sampler2D u_tex;

in vec2 f_tex_coord;
in vec2 f_pos;

out vec4 out_color;

float sdroundrect(vec2 pt, vec2 ext, float rad) {
    vec2 ext2 = ext - vec2(rad, rad);
    vec2 d = abs(pt) - ext2;
    return min(max(d.x, d.y), 0.0) + length(max(d, 0.0)) - rad;
}

float scissor_mask(vec2 p) {
    vec2 sc = (abs((scissor_mat * vec3(p, 1.0)).xy) - scissor_ext);
    sc = vec2(0.5, 0.5) - sc * scissor_scale;
    return clamp(sc.x, 0.0, 1.0) * clamp(sc.y, 0.0, 1.0);
}

#ifdef EDGE_AA
// Stroke - from [0..1] to clipped pyramid, where the slope is 1px.
float stroke_mask() {
    return min(1.0, (1.0 - abs(f_tex_coord.x * 2.0 - 1.0)) * stroke_mult) * min(1.0, f_tex_coord.y);
}
#endif

void main() {
    #ifdef EDGE_AA
        float stroke_alpha = stroke_mask();
        if (stroke_alpha < stroke_thr) {
            discard;
        }
    #else
        float stroke_alpha = 1.0;
    #endif

    vec4 result;
    float scissor = scissor_mask(f_pos);

    if (type == 0) { // Gradient
        vec2 pt = (paint_mat * vec3(f_pos, 1.0)).xy;
        float d = clamp((sdroundrect(pt, extent, radius) + feather * 0.5) / feather, 0.0, 1.0);
        vec4 color = mix(inner_col, outer_col, d);
        color *= stroke_alpha * scissor;
        result = color;
    } else if (type == 1) { // image
    } else if (type == 2) { // Stencil fill
		result = vec4(1,1,1,1);
    } else if (type == 3) { // Textured tris
    }

    out_color = result;
}
