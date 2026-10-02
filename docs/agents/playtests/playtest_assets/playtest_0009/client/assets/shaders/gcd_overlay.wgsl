// Radial "cooldown sweep" overlay for hotbar slots: a dark pie-slice wipe, centered on the node,
// that shrinks clockwise from fully covered (GCD just triggered) to fully clear (GCD ready).
#import bevy_ui::ui_vertex_output::UiVertexOutput

const TAU: f32 = 6.28318530718;

// .x = fraction of the GCD still remaining, 1.0 (just triggered) down to 0.0 (ready).
@group(1) @binding(0) var<uniform> covered: vec4<f32>;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let d = in.uv - vec2(0.5, 0.5);
    // Angle clockwise from the top (12 o'clock), in [0, TAU).
    var angle = atan2(d.x, -d.y);
    if angle < 0.0 {
        angle = angle + TAU;
    }
    if angle < covered.x * TAU {
        return vec4(0.0, 0.0, 0.0, 0.65);
    }
    return vec4(0.0, 0.0, 0.0, 0.0);
}
