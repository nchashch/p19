// Minimalist white radial progress ring for the crosshair while the GCD is running (replaces the
// plain dot — see `hud.rs`'s `update_crosshair_gcd`). Same clockwise-from-12-o'clock sweep
// direction as `gcd_overlay.wgsl`'s hotbar pie wipe, just rendered as a thin ring instead of a
// filled square, and depleting (shrinking away to nothing right as the GCD finishes) rather than
// covering, so the dot can take back over at exactly the moment this empties out.
#import bevy_ui::ui_vertex_output::UiVertexOutput

const TAU: f32 = 6.28318530718;
// Ring thickness as a fraction of the node's radius — scales with whatever size the node is given.
const RING_THICKNESS: f32 = 0.22;

// .x = fraction of the GCD still remaining, 1.0 (just triggered) down to 0.0 (ready).
@group(1) @binding(0) var<uniform> covered: vec4<f32>;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let d = in.uv - vec2(0.5, 0.5);
    let dist = length(d) * 2.0; // 0 at the center, 1 at the node's inscribed circle edge
    if dist > 1.0 || dist < 1.0 - RING_THICKNESS {
        return vec4(0.0, 0.0, 0.0, 0.0);
    }
    // Angle clockwise from the top (12 o'clock), in [0, TAU).
    var angle = atan2(d.x, -d.y);
    if angle < 0.0 {
        angle = angle + TAU;
    }
    if angle < covered.x * TAU {
        return vec4(1.0, 1.0, 1.0, 1.0);
    }
    return vec4(0.0, 0.0, 0.0, 0.0);
}
