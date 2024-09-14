struct TimerUniform {
  t: f32
};
@group(0) @binding(0)
var<uniform> timer: TimerUniform;


struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.color.x = model.color.x + cos(model.position.x) + sin(timer.t);
    out.color.y = model.color.y + sin(model.position.x) + cos(timer.t);
    out.color.z = model.color.z + cos(model.position.x) + cos(timer.t);
    out.clip_position = vec4<f32>(model.position, 1.0);
    return out;
}

// Fragment shader
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
