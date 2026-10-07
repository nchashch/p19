//! A horizontal slider element for `bevy_markup` templates, which have no range input:
//!
//! ```html
//! <div class="slider" id="…" is="slider" data-slider="<key>" data-on-click="slider.activate">
//!   <div class="slider-fill" style="width: {{ percent }}%"></div>
//!   <div class="slider-thumb" style="left: {{ percent }}%"></div>
//! </div>
//! ```
//!
//! The app owns the value, its range and its step; the template draws it from the context, so the
//! element's `data-*` stays constant and in-place updates keep the entity (and a drag in
//! progress). Input arrives as [`SliderInput`] messages keyed by `data-slider`:
//! - **pointer:** primary press on the track, then dragging (anywhere, until release), reports
//!   [`SliderChange::Set`] with the pointer's fraction along the track;
//! - **keyboard / gamepad:** the slider is focusable (`data-on-click`; activating it does
//!   nothing), and while it's focused, left/right `UiNavigate` (with its auto-repeat) report
//!   [`SliderChange::Step`] instead of moving focus — see [`step_for`].

use bevy::math::CompassOctant;
use bevy::picking::events::{Drag, Pointer, Press};
use bevy::picking::pointer::{Location, PointerButton};
use bevy::prelude::*;
use bevy::ui::UiGlobalTransform;
use bevy_markup::prelude::*;

pub struct SliderPlugin;

impl Plugin for SliderPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SliderInput>()
            .define_html_element("slider", connect_slider);
    }
}

/// A connected slider element; `key` is its `data-slider`.
#[derive(Component)]
pub struct Slider {
    pub key: String,
}

/// Input on a slider, for the app to apply to its value.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct SliderInput {
    /// The slider's `data-slider`.
    pub key: String,
    pub change: SliderChange,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SliderChange {
    /// Pointer at this fraction of the track, `0.0` (left end) to `1.0` (right end).
    Set(f32),
    /// One step right (`+1`) or left (`-1`).
    Step(i32),
}

fn connect_slider(element: In<ElementConnected>, mut commands: Commands) {
    let Some(key) = element.data("slider") else {
        warn!("`is=\"slider\"` element without a `data-slider` key");
        return;
    };
    let slider = element.entity;
    commands
        .entity(slider)
        .insert(Slider {
            key: key.to_owned(),
        })
        .observe(
            move |press: On<Pointer<Press>>,
                  sliders: Query<(&Slider, &ComputedNode, &UiGlobalTransform)>,
                  mut input: MessageWriter<SliderInput>| {
                if press.event.button == PointerButton::Primary {
                    set_from_pointer(slider, &press.pointer_location, &sliders, &mut input);
                }
            },
        )
        .observe(
            move |drag: On<Pointer<Drag>>,
                  sliders: Query<(&Slider, &ComputedNode, &UiGlobalTransform)>,
                  mut input: MessageWriter<SliderInput>| {
                if drag.event.button == PointerButton::Primary {
                    set_from_pointer(slider, &drag.pointer_location, &sliders, &mut input);
                }
            },
        );
}

/// The pointer's fraction along the slider's width, clamped — the same physical-pixel mapping
/// `bevy_ui`'s picking backend uses (`ComputedNode::normalize_point`).
fn set_from_pointer(
    slider: Entity,
    location: &Location,
    sliders: &Query<(&Slider, &ComputedNode, &UiGlobalTransform)>,
    input: &mut MessageWriter<SliderInput>,
) {
    let Ok((slider, node, transform)) = sliders.get(slider) else {
        return;
    };
    let physical = location.position / node.inverse_scale_factor;
    if let Some(point) = node.normalize_point(*transform, physical) {
        input.write(SliderInput {
            key: slider.key.clone(),
            change: SliderChange::Set((point.x + 0.5).clamp(0.0, 1.0)),
        });
    }
}

/// The step a `UiNavigate` direction makes on the focused slider, or `None` when focus isn't on
/// a slider or the direction isn't left/right (then it navigates as usual).
pub fn step_for(
    focused: Option<Entity>,
    direction: CompassOctant,
    sliders: &Query<&Slider>,
) -> Option<SliderInput> {
    let step = match direction {
        CompassOctant::East => 1,
        CompassOctant::West => -1,
        _ => return None,
    };
    let slider = sliders.get(focused?).ok()?;
    Some(SliderInput {
        key: slider.key.clone(),
        change: SliderChange::Step(step),
    })
}
